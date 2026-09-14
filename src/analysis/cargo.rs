//! Cargo manifest and lockfile dependency view.
//!
//! This is the first analysis in the tree that parses a structured document
//! format the repository author controls, rather than reading bytes, names,
//! or a subprocess's own stable output. That brings a new failure mode
//! (syntactically valid but semantically surprising input) and a new leak
//! channel: **a parser's own error text quotes the input back**.
//!
//! Both routes from a `toml::de::Error` to a string were measured, and both
//! leak. `Display` echoes the offending source line verbatim, ANSI escapes
//! and all — a live escape sequence on a path to the terminal (invariant
//! I4). `Error::message()` looks like the safe alternative and is not:
//! serde's own type-mismatch errors embed the offending value (e.g.
//! `invalid type: string "sk_live_SECRET_VALUE", expected u32`). **Neither
//! is ever put into the report, a warning, or any output.** Only
//! [`toml::de::Error::span`] — a byte range with no content in it — is used,
//! converted to a 1-based line number by this module's own code. The
//! `NotEvaluated` detail is always a tool-authored constant plus that line
//! number. This is the same rule `src/analysis/git.rs` applies to git's
//! stderr, arrived at independently: diagnostic text from a parser or
//! subprocess is untrusted content, not a message.
//!
//! Every dependency name, version requirement, and target `cfg(...)`
//! expression this module produces is untrusted repository content, the
//! same way a file name is. This module stores what the manifest said, only
//! sanitizing on render — `sanitize_for_terminal` in the text renderer,
//! [`crate::render::html::Html::escape`] in the HTML renderer — never at
//! parse time, which would corrupt the JSON contract (values there are
//! already made safe by JSON string encoding).
//!
//! Both files are read from `root.join("Cargo.toml")` and
//! `root.join("Cargo.lock")` — fixed names joined to the scanned root, never
//! a path taken from file content, so no traversal is possible (invariant
//! I8). Nothing here is executed (invariant I3) and nothing opens a socket
//! (invariant I6). Each file is capped at 4 MiB, checked with
//! [`std::fs::metadata`] *before* it is read, so a hostile repository cannot
//! turn a read into an unbounded allocation (invariant I9). Every failure —
//! missing file, oversized file, non-UTF-8 content, invalid TOML, or TOML
//! that does not match a Cargo manifest's shape — degrades to
//! [`crate::NotEvaluated`] rather than aborting the scan or panicking
//! (invariant I10); [`analyze`] never returns an error.

use std::collections::BTreeMap;
use std::path::Path;
// `read_bounded`'s own body moved to `super::read_bounded`, so this module no
// longer touches `std::fs` outside its tests, which still exercise the size
// cap directly against the filesystem.
#[cfg(test)]
use std::fs;

use serde::{Deserialize, Serialize};

use crate::{Analysis, NotEvaluated, ScanConfig};

/// Files larger than this are refused before they are read, so a hostile
/// repository cannot turn a manifest or lockfile read into an unbounded
/// allocation (invariant I9). The largest manifest in a 2026-09-13 sample of
/// the local registry cache was far under this.
///
/// `pub(crate)`: `src/analysis/profile.rs` reuses this same cap for the
/// manifests it reads, per the build sheet's bounded-reads table.
pub(crate) const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// What the manifest at the scanned root declares.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CargoManifest {
    /// The package name, absent for a virtual workspace manifest.
    pub package_name: Option<String>,
    /// The package version, absent for a virtual workspace manifest.
    pub package_version: Option<String>,
    /// True when the manifest declares a `[workspace]` table.
    pub workspace_root: bool,
    /// Direct dependencies, sorted by kind then name. Untrusted content:
    /// every string here came from the manifest and must be sanitized
    /// before it reaches a terminal.
    pub dependencies: Vec<CargoDependency>,
}

/// One declared dependency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CargoDependency {
    /// The dependency name as written in the manifest.
    pub name: String,
    /// The declared version requirement. Absent for a git, path, or
    /// workspace-inherited dependency, which state no version of their own.
    pub requirement: Option<String>,
    /// Which dependency table this came from.
    pub kind: DependencyKind,
    /// The `cfg(...)` expression, when the dependency is target-specific.
    pub target: Option<String>,
    /// Where the dependency resolves from.
    pub source: DependencySource,
}

/// Which dependency table a dependency was declared in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyKind {
    /// `[dependencies]`.
    Normal,
    /// `[dev-dependencies]`.
    Dev,
    /// `[build-dependencies]`.
    Build,
}

impl DependencyKind {
    /// The stable lowercase token for this kind, matching the JSON
    /// contract's `rename_all = "lowercase"` spelling.
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Dev => "dev",
            Self::Build => "build",
        }
    }
}

/// Where a dependency resolves from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencySource {
    /// A registry such as crates.io. The default when nothing else applies.
    Registry,
    /// A `git` dependency.
    Git,
    /// A local `path` dependency.
    Path,
    /// `workspace = true`: the version is inherited from the workspace.
    Workspace,
}

impl DependencySource {
    /// The stable lowercase token for this source, matching the JSON
    /// contract's `rename_all = "lowercase"` spelling.
    pub fn label(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::Git => "git",
            Self::Path => "path",
            Self::Workspace => "workspace",
        }
    }
}

/// What the lockfile resolved to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CargoLock {
    /// The lockfile format version, when it declares one.
    pub lock_version: Option<u32>,
    /// Total packages in the lockfile.
    pub packages: usize,
    /// Packages with no `source` — workspace members and path dependencies,
    /// which are part of this repository rather than fetched.
    pub local_packages: usize,
}

// `NotEvaluated` detail text is always built from these pieces plus a line
// number, never from `toml::de::Error`'s own `Display` or `message()` — see
// the module doc for why both leak repository content.
const MANIFEST_NAME: &str = "Cargo.toml";
const LOCK_NAME: &str = "Cargo.lock";

/// Reads the Cargo manifest and lockfile at `root`.
///
/// Never returns an error: a missing, oversized, non-UTF-8, or malformed
/// file becomes a `NotEvaluated` reason, so a non-Cargo repository still
/// produces a successful report. The manifest and lockfile are read
/// independently, so either can be `Ran` while the other is not — a
/// manifest with no committed lockfile is an ordinary repository, not a
/// failure.
pub fn analyze(root: &Path, config: &ScanConfig) -> (Analysis<CargoManifest>, Analysis<CargoLock>) {
    if !config.read_cargo {
        return (
            Analysis::NotEvaluated(NotEvaluated::Disabled),
            Analysis::NotEvaluated(NotEvaluated::Disabled),
        );
    }

    (manifest_analysis(root), lock_analysis(root))
}

fn manifest_analysis(root: &Path) -> Analysis<CargoManifest> {
    let content = match read_bounded(&root.join(MANIFEST_NAME), MANIFEST_NAME) {
        Ok(content) => content,
        Err(reason) => return Analysis::NotEvaluated(reason),
    };

    match parse_manifest(&content) {
        Ok(manifest) => Analysis::Ran(manifest),
        Err(reason) => Analysis::NotEvaluated(reason),
    }
}

fn lock_analysis(root: &Path) -> Analysis<CargoLock> {
    let content = match read_bounded(&root.join(LOCK_NAME), LOCK_NAME) {
        Ok(content) => content,
        Err(reason) => return Analysis::NotEvaluated(reason),
    };

    match parse_lock(&content) {
        Ok(lock) => Analysis::Ran(lock),
        Err(reason) => Analysis::NotEvaluated(reason),
    }
}

/// Reads `path` as UTF-8 text, bounded at [`MAX_FILE_BYTES`]. A thin,
/// same-arity wrapper over the shared [`super::read_bounded`] (moved there
/// so `src/analysis/profile.rs` can reuse it with its own caps) — kept here
/// so every call site in this module, and its existing tests, need no
/// change. The four detail strings this produces are byte-identical to the
/// ones the private copy used to build directly.
fn read_bounded(path: &Path, label: &str) -> Result<String, NotEvaluated> {
    super::read_bounded(path, label, MAX_FILE_BYTES)
}

/// Deserialize with `serde`. All five dependency-declaration shapes parse
/// through the untagged [`RawDependency`] enum: a bare version string, an
/// inline table, a workspace-inherited table, a git table, and the
/// `[dependencies.name]` table form.
///
/// Unknown tables and fields (`[lints]`, `[features]`, `[[bench]]`, and
/// whatever Cargo adds next) are ignored deliberately — `deny_unknown_fields`
/// would fail on any ordinary manifest that carries one.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawManifest {
    package: Option<RawPackage>,
    workspace: Option<toml::Value>,
    #[serde(default)]
    dependencies: BTreeMap<String, RawDependency>,
    #[serde(default)]
    dev_dependencies: BTreeMap<String, RawDependency>,
    #[serde(default)]
    build_dependencies: BTreeMap<String, RawDependency>,
    #[serde(default)]
    target: BTreeMap<String, RawTarget>,
}

#[derive(Debug, Deserialize)]
struct RawPackage {
    name: Option<String>,
    version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawTarget {
    #[serde(default)]
    dependencies: BTreeMap<String, RawDependency>,
    #[serde(default)]
    dev_dependencies: BTreeMap<String, RawDependency>,
    #[serde(default)]
    build_dependencies: BTreeMap<String, RawDependency>,
}

/// A single dependency declaration, in any of the shapes Cargo accepts.
/// Verified to cover a bare version string, an inline detailed table, a
/// `workspace = true` table, a `git` table, and the `[dependencies.name]`
/// table form (2026-09-13, against this crate's own toolchain).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawDependency {
    Version(String),
    Detailed(RawDetailed),
}

/// The fields this module reads out of a detailed dependency table. Other
/// real fields (`features`, `optional`, `default-features`, `branch`, `tag`,
/// `rev`, `registry`, `package`, ...) are present in real manifests and are
/// ignored the same way an unknown top-level table is.
#[derive(Debug, Default, Deserialize)]
struct RawDetailed {
    version: Option<String>,
    git: Option<String>,
    path: Option<String>,
    workspace: Option<bool>,
}

/// Builds the reported [`CargoManifest`] from what actually deserialized.
fn build_manifest(raw: RawManifest) -> CargoManifest {
    let package_name = raw
        .package
        .as_ref()
        .and_then(|package| package.name.clone());
    let package_version = raw
        .package
        .as_ref()
        .and_then(|package| package.version.clone());
    let workspace_root = raw.workspace.is_some();

    let mut dependencies = Vec::new();
    push_dependencies(
        &mut dependencies,
        raw.dependencies,
        DependencyKind::Normal,
        None,
    );
    push_dependencies(
        &mut dependencies,
        raw.dev_dependencies,
        DependencyKind::Dev,
        None,
    );
    push_dependencies(
        &mut dependencies,
        raw.build_dependencies,
        DependencyKind::Build,
        None,
    );
    for (cfg, target) in raw.target {
        push_dependencies(
            &mut dependencies,
            target.dependencies,
            DependencyKind::Normal,
            Some(&cfg),
        );
        push_dependencies(
            &mut dependencies,
            target.dev_dependencies,
            DependencyKind::Dev,
            Some(&cfg),
        );
        push_dependencies(
            &mut dependencies,
            target.build_dependencies,
            DependencyKind::Build,
            Some(&cfg),
        );
    }

    // `BTreeMap` gives name order within one table, but the three tables are
    // concatenated and target-specific entries are appended afterward, so
    // the combined order is not free — this explicit sort is
    // ENGINEERING.md's determinism rule, not a formatting nicety.
    dependencies.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.name.cmp(&right.name))
    });

    CargoManifest {
        package_name,
        package_version,
        workspace_root,
        dependencies,
    }
}

fn push_dependencies(
    out: &mut Vec<CargoDependency>,
    table: BTreeMap<String, RawDependency>,
    kind: DependencyKind,
    target: Option<&str>,
) {
    for (name, dependency) in table {
        let (requirement, source) = match dependency {
            RawDependency::Version(version) => (Some(version), DependencySource::Registry),
            RawDependency::Detailed(detailed) => {
                let source = if detailed.workspace == Some(true) {
                    DependencySource::Workspace
                } else if detailed.git.is_some() {
                    DependencySource::Git
                } else if detailed.path.is_some() {
                    DependencySource::Path
                } else {
                    DependencySource::Registry
                };
                (detailed.version, source)
            }
        };

        out.push(CargoDependency {
            name,
            requirement,
            kind,
            target: target.map(str::to_owned),
            source,
        });
    }
}

/// Parses `content` as a Cargo manifest.
///
/// Two passes, deliberately: the first checks only that `content` is
/// syntactically valid TOML at all (`toml::Value` accepts any table shape),
/// so a syntax error is reported as `"{MANIFEST_NAME} is not valid TOML"`.
/// Only once that passes does the second attempt the real
/// [`RawManifest`] shape, so a value of the wrong type is reported as
/// `"{MANIFEST_NAME} is not a valid Cargo manifest"` instead — a different,
/// still tool-authored, detail for a different condition. Both messages
/// carry a line number from [`toml::de::Error::span`] and nothing else; see
/// the module doc for why `Display` and `message()` are never used.
fn parse_manifest(content: &str) -> Result<CargoManifest, NotEvaluated> {
    if let Err(error) = toml::from_str::<toml::Value>(content) {
        return Err(NotEvaluated::Failed(format!(
            "{MANIFEST_NAME} is not valid TOML{}",
            line_suffix(content, &error)
        )));
    }

    let raw = toml::from_str::<RawManifest>(content).map_err(|error| {
        NotEvaluated::Failed(format!(
            "{MANIFEST_NAME} is not a valid Cargo manifest{}",
            line_suffix(content, &error)
        ))
    })?;

    Ok(build_manifest(raw))
}

/// The lockfile's own shape: a format version and an array of resolved
/// packages. Verified against this repository's own `Cargo.lock`
/// (2026-09-13): `version: Some(4)`, and `source: None` identifies a local
/// package (a workspace member or path dependency) rather than a fetched
/// one. `name` and `version` are not modelled here — spec 003's 4c parcel
/// reports counts only; a per-package resolved version is 017's model to
/// design, not this one's to anticipate.
#[derive(Debug, Deserialize)]
struct RawLock {
    version: Option<u32>,
    #[serde(default)]
    package: Vec<RawLockPackage>,
}

#[derive(Debug, Deserialize)]
struct RawLockPackage {
    source: Option<String>,
}

fn build_lock(raw: RawLock) -> CargoLock {
    let packages = raw.package.len();
    let local_packages = raw
        .package
        .iter()
        .filter(|package| package.source.is_none())
        .count();

    CargoLock {
        lock_version: raw.version,
        packages,
        local_packages,
    }
}

/// Parses `content` as a Cargo lockfile. Mirrors [`parse_manifest`]'s
/// two-pass structure and the same rule about what a `toml::de::Error` may
/// contribute to the detail string: a line number, nothing else.
fn parse_lock(content: &str) -> Result<CargoLock, NotEvaluated> {
    if let Err(error) = toml::from_str::<toml::Value>(content) {
        return Err(NotEvaluated::Failed(format!(
            "{LOCK_NAME} is not valid TOML{}",
            line_suffix(content, &error)
        )));
    }

    let raw = toml::from_str::<RawLock>(content).map_err(|error| {
        NotEvaluated::Failed(format!(
            "{LOCK_NAME} is not a valid Cargo lockfile{}",
            line_suffix(content, &error)
        ))
    })?;

    Ok(build_lock(raw))
}

/// Formats `" (line N)"` from a `toml::de::Error`'s [`toml::de::Error::span`],
/// or an empty string when no span is available. This is the *only* thing
/// this module ever reads off a `toml::de::Error` — never `Display`, never
/// `message()`. Both were measured to echo repository content (see the
/// module doc); `span()` is a byte range with no content in it.
fn line_suffix(content: &str, error: &toml::de::Error) -> String {
    match error.span() {
        Some(span) => format!(" (line {})", line_number(content, span.start)),
        None => String::new(),
    }
}

/// Converts a byte offset into a 1-based line number by counting newlines in
/// `content` up to `offset`. `content.get` guards against an offset that is
/// not a char boundary rather than slicing and possibly panicking — a
/// defensive fallback for invariant I9, not a case expected to trigger in
/// practice against a `toml`-reported span.
fn line_number(content: &str, offset: usize) -> usize {
    let bounded = offset.min(content.len());
    let prefix = content.get(..bounded).unwrap_or(content);
    prefix.matches('\n').count() + 1
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn manifest(content: &str) -> CargoManifest {
        parse_manifest(content).expect("manifest should parse")
    }

    fn dependency<'a>(manifest: &'a CargoManifest, name: &str) -> &'a CargoDependency {
        manifest
            .dependencies
            .iter()
            .find(|dependency| dependency.name == name)
            .unwrap_or_else(|| panic!("dependency {name} should be present"))
    }

    #[test]
    fn parses_every_dependency_declaration_shape() {
        let manifest = manifest(
            r#"
            [package]
            name = "fixture"
            version = "0.1.0"

            [dependencies]
            simple = "1"
            inline = { version = "2", features = ["a"] }
            inherited = { workspace = true }
            gitdep = { git = "https://example.invalid/repo.git" }

            [dependencies.tableform]
            version = "3"
            "#,
        );

        let simple = dependency(&manifest, "simple");
        assert_eq!(simple.requirement.as_deref(), Some("1"));
        assert_eq!(simple.source, DependencySource::Registry);

        let inline = dependency(&manifest, "inline");
        assert_eq!(inline.requirement.as_deref(), Some("2"));
        assert_eq!(inline.source, DependencySource::Registry);

        let inherited = dependency(&manifest, "inherited");
        assert_eq!(inherited.requirement, None);
        assert_eq!(inherited.source, DependencySource::Workspace);

        let gitdep = dependency(&manifest, "gitdep");
        assert_eq!(gitdep.requirement, None);
        assert_eq!(gitdep.source, DependencySource::Git);

        let tableform = dependency(&manifest, "tableform");
        assert_eq!(tableform.requirement.as_deref(), Some("3"));
        assert_eq!(tableform.source, DependencySource::Registry);
    }

    #[test]
    fn parses_dependency_kinds() {
        let manifest = manifest(
            r#"
            [dependencies]
            normal-dep = "1"

            [dev-dependencies]
            dev-dep = "1"

            [build-dependencies]
            build-dep = "1"
            "#,
        );

        assert_eq!(
            dependency(&manifest, "normal-dep").kind,
            DependencyKind::Normal
        );
        assert_eq!(dependency(&manifest, "dev-dep").kind, DependencyKind::Dev);
        assert_eq!(
            dependency(&manifest, "build-dep").kind,
            DependencyKind::Build
        );
    }

    #[test]
    fn parses_target_specific_dependencies() {
        let manifest = manifest(
            r#"
            [target.'cfg(unix)'.dependencies]
            libc = "0.2"
            "#,
        );

        let libc = dependency(&manifest, "libc");
        assert_eq!(libc.target.as_deref(), Some("cfg(unix)"));
        assert_eq!(libc.kind, DependencyKind::Normal);
    }

    #[test]
    fn dependencies_are_sorted_by_kind_then_name() {
        let manifest = manifest(
            r#"
            [build-dependencies]
            zebra = "1"

            [dependencies]
            zebra = "1"
            apple = "1"

            [dev-dependencies]
            middle = "1"
            "#,
        );

        let ordered: Vec<(DependencyKind, &str)> = manifest
            .dependencies
            .iter()
            .map(|dependency| (dependency.kind, dependency.name.as_str()))
            .collect();

        assert_eq!(
            ordered,
            vec![
                (DependencyKind::Normal, "apple"),
                (DependencyKind::Normal, "zebra"),
                (DependencyKind::Dev, "middle"),
                (DependencyKind::Build, "zebra"),
            ],
            "deliberately unsorted input must still produce (kind, name) order"
        );
    }

    #[test]
    fn virtual_workspace_manifest_has_no_package() {
        let manifest = manifest(
            r#"
            [workspace]
            members = ["crates/*"]
            "#,
        );

        assert_eq!(manifest.package_name, None);
        assert_eq!(manifest.package_version, None);
        assert!(manifest.workspace_root);
    }

    #[test]
    fn unknown_tables_are_ignored() {
        let manifest = manifest(
            r#"
            [package]
            name = "fixture"

            [lints]
            workspace = true

            [features]
            default = []

            [invented-table]
            whatever = "value"
            "#,
        );

        assert_eq!(manifest.package_name.as_deref(), Some("fixture"));
    }

    #[test]
    fn lockfile_counts_packages_and_local_packages() {
        let lock = parse_lock(
            r#"
            version = 4

            [[package]]
            name = "fetched"
            version = "1.0.0"
            source = "registry+https://github.com/rust-lang/crates.io-index"

            [[package]]
            name = "workspace-member"
            version = "0.1.0"
            "#,
        )
        .expect("lockfile should parse");

        assert_eq!(lock.lock_version, Some(4));
        assert_eq!(lock.packages, 2);
        assert_eq!(lock.local_packages, 1);
    }

    #[test]
    fn malformed_toml_reports_the_line_and_nothing_else() {
        // Verbatim from the build sheet: an unterminated value carrying a
        // live ANSI escape and a fake secret. `toml::de::Error`'s `Display`
        // echoes this line back exactly, escape byte and all — this test
        // exists to prove that text never reaches the report.
        let content =
            "[package]\nname = \"ok\"\nevil = \u{1b}[31mAPI_KEY_sk_live_abc123 unterminated\n";

        let error = parse_manifest(content).expect_err("malformed TOML must not parse");
        let NotEvaluated::Failed(detail) = error else {
            panic!("a parse failure must report Failed, got {error:?}");
        };

        assert!(
            detail.contains("line 3"),
            "detail should name the offending line: {detail:?}"
        );
        for forbidden in ["API_KEY", "sk_live", "\u{1b}"] {
            assert!(
                !detail.contains(forbidden),
                "detail leaked repository content ({forbidden:?}): {detail:?}"
            );
        }
    }

    #[test]
    fn type_error_does_not_leak_the_offending_value() {
        // Syntactically valid TOML; `package` is a string where a table is
        // expected. This specific shape matters: serde's `invalid_type`
        // error embeds the scalar value verbatim in its message (measured:
        // `invalid type: string "sk_live_SECRET_VALUE", expected struct
        // RawPackage`) — `Error::message()` is the leak channel that looks
        // safe and is not. A mismatch *inside* an untagged enum (e.g. a
        // dependency table's own wrong-typed field) does not reproduce this:
        // `serde`'s untagged handling discards each variant's own message
        // and reports only "data did not match any variant", so the fixture
        // has to be a direct, non-enum field to actually exercise the leak
        // this test guards against.
        let content = "package = \"sk_live_SECRET_VALUE\"\n";

        let error = parse_manifest(content).expect_err("a string is not a valid package table");
        let NotEvaluated::Failed(detail) = error else {
            panic!("a shape failure must report Failed, got {error:?}");
        };

        assert!(
            !detail.contains("sk_live_SECRET_VALUE"),
            "detail leaked the offending value: {detail:?}"
        );
    }

    #[test]
    fn oversized_manifest_is_rejected_before_parsing() {
        let fixture_dir =
            std::env::temp_dir().join(format!("repo-radar-cargo-oversized-{}", std::process::id()));
        fs::create_dir_all(&fixture_dir).expect("fixture directory should be created");
        let path = fixture_dir.join("Cargo.toml");

        // Content that would fail to parse as TOML if it were ever read, so
        // a detail naming a parse failure instead of the size limit would
        // prove the size check did not fire first.
        let oversized = vec![b'{'; (MAX_FILE_BYTES + 1) as usize];
        fs::write(&path, &oversized).expect("oversized fixture should be written");

        let result = read_bounded(&path, MANIFEST_NAME);

        fs::remove_dir_all(&fixture_dir).ok();

        match result {
            Err(NotEvaluated::Failed(detail)) => {
                assert_eq!(detail, "Cargo.toml exceeds the size limit");
            }
            other => panic!("expected a size-limit failure, got {other:?}"),
        }
    }
}
