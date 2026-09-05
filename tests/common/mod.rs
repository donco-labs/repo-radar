//! Shared test support for the safety invariants in
//! `docs/specs/000-safety-invariants.md`.
//!
//! The core of this module is [`TreeDigest`]: a recursive fingerprint of a
//! directory covering contents, sizes, modification times, permissions, and
//! symlink targets. Capturing one before and after a command, then asserting
//! equality, is how invariant I1 — the inspected repository is immutable — is
//! held to something stronger than an intention.
//!
//! Access times are deliberately excluded. Reading a file updates its atime on
//! many platforms, so including it would make every read look like a mutation.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// What a single filesystem entry looked like at digest time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDigest {
    pub kind: &'static str,
    pub bytes: Option<u64>,
    pub content_hash: Option<u64>,
    pub modified_nanos: Option<u128>,
    pub mode: Option<u32>,
    pub link_target: Option<PathBuf>,
}

/// A recursive fingerprint of a directory tree, keyed by relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeDigest {
    entries: BTreeMap<PathBuf, EntryDigest>,
}

impl TreeDigest {
    /// Fingerprints `root`, including hidden files and the `.git` directory.
    ///
    /// Symbolic links are recorded by their target and never followed, so a
    /// link pointing outside the tree cannot pull unrelated state into the
    /// digest or cause an infinite descent.
    pub fn capture(root: &Path) -> Self {
        let mut entries = BTreeMap::new();
        collect(root, root, &mut entries);
        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Describes every difference against `other`, or `None` if identical.
    ///
    /// The report names the specific paths and fields that moved, because a
    /// bare "trees differ" failure would leave the reader to hunt for the
    /// mutation by hand.
    pub fn difference(&self, other: &Self) -> Option<String> {
        let mut report = String::new();

        for (path, before) in &self.entries {
            match other.entries.get(path) {
                None => {
                    let _ = writeln!(report, "  removed: {}", path.display());
                }
                Some(after) if after != before => {
                    let _ = writeln!(report, "  changed: {}", path.display());
                    describe_change(&mut report, before, after);
                }
                Some(_) => {}
            }
        }

        for path in other.entries.keys() {
            if !self.entries.contains_key(path) {
                let _ = writeln!(report, "  created: {}", path.display());
            }
        }

        (!report.is_empty()).then_some(report)
    }
}

fn describe_change(report: &mut String, before: &EntryDigest, after: &EntryDigest) {
    let mut note = |field: &str, before: String, after: String| {
        if before != after {
            let _ = writeln!(report, "    {field}: {before} -> {after}");
        }
    };

    note("kind", before.kind.to_owned(), after.kind.to_owned());
    note(
        "bytes",
        format!("{:?}", before.bytes),
        format!("{:?}", after.bytes),
    );
    note(
        "content",
        format!("{:?}", before.content_hash),
        format!("{:?}", after.content_hash),
    );
    note(
        "modified",
        format!("{:?}", before.modified_nanos),
        format!("{:?}", after.modified_nanos),
    );
    note(
        "mode",
        format!("{:?}", before.mode),
        format!("{:?}", after.mode),
    );
    note(
        "link target",
        format!("{:?}", before.link_target),
        format!("{:?}", after.link_target),
    );
}

fn collect(root: &Path, directory: &Path, entries: &mut BTreeMap<PathBuf, EntryDigest>) {
    let Ok(read_dir) = fs::read_dir(directory) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };

        let modified_nanos = metadata.modified().ok().and_then(|time| {
            time.duration_since(UNIX_EPOCH)
                .ok()
                .map(|since| since.as_nanos())
        });

        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            Some(metadata.permissions().mode())
        };
        #[cfg(not(unix))]
        let mode = None;

        let digest = if metadata.is_symlink() {
            EntryDigest {
                kind: "symlink",
                bytes: None,
                content_hash: None,
                modified_nanos,
                mode,
                link_target: fs::read_link(&path).ok(),
            }
        } else if metadata.is_dir() {
            collect(root, &path, entries);
            EntryDigest {
                kind: "dir",
                bytes: None,
                content_hash: None,
                modified_nanos,
                mode,
                link_target: None,
            }
        } else {
            EntryDigest {
                kind: "file",
                bytes: Some(metadata.len()),
                content_hash: fs::read(&path).ok().map(|contents| {
                    let mut hasher = DefaultHasher::new();
                    contents.hash(&mut hasher);
                    hasher.finish()
                }),
                modified_nanos,
                mode,
                link_target: None,
            }
        };

        entries.insert(relative, digest);
    }
}

/// Runs `operation` and fails the test if it changed anything under `root`.
///
/// This is the primary entry point for invariant I1. Every command-level test,
/// including every failure-path test, is expected to run inside it.
pub fn assert_target_unchanged<T>(
    root: &Path,
    description: &str,
    operation: impl FnOnce() -> T,
) -> T {
    let before = TreeDigest::capture(root);
    assert!(
        !before.is_empty(),
        "fixture at {} is empty, so an immutability assertion over it would prove nothing",
        root.display()
    );

    let result = operation();
    let after = TreeDigest::capture(root);

    if let Some(difference) = before.difference(&after) {
        panic!(
            "{description} modified the inspected repository at {}.\n\
             Repo Radar treats the target as immutable (spec 000, invariant I1).\n{difference}",
            root.display()
        );
    }

    result
}

/// A temporary directory tree that deletes itself when dropped.
pub struct Fixture {
    pub root: PathBuf,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

impl Fixture {
    pub fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("repo-radar-inv-{suffix}-{counter}"));
        fs::create_dir_all(&root).expect("fixture root should be created");
        Self { root }
    }

    /// Writes a file, creating parent directories as needed.
    pub fn file(&self, relative: &str, contents: &[u8]) -> &Self {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent should be created");
        }
        fs::write(&path, contents).expect("fixture file should be written");
        self
    }

    pub fn dir(&self, relative: &str) -> &Self {
        fs::create_dir_all(self.root.join(relative)).expect("fixture directory should be created");
        self
    }

    /// A fixture exercising the shapes a scanner tends to get wrong.
    pub fn typical() -> Self {
        let fixture = Self::new();
        fixture
            .file("Cargo.toml", b"[package]\nname = \"fixture\"\n")
            .file("README.md", b"# Fixture\n")
            .file("src/main.rs", b"fn main() {}\n")
            .file("src/lib.rs", b"pub fn work() {}\n")
            .file("docs/guide.md", b"guide\n")
            .file("target/build-artifact.rs", b"ignored\n")
            .file("node_modules/dep/index.js", b"ignored\n")
            .file("no-extension", b"data\n")
            .file("empty.rs", b"")
            .dir("empty-directory");
        fixture
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Runs the Repo Radar binary with the given arguments.
pub fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repo-radar"))
        .args(arguments)
        .output()
        .expect("repo-radar should run")
}

/// Runs Git with a fixed argument vector and a deterministic identity.
///
/// Arguments are passed as a vector rather than through a shell, upholding
/// invariant I4. Returns `None` when Git is unavailable so tests can skip
/// rather than fail on a machine without it.
pub fn git(directory: &Path, arguments: &[&str]) -> Option<Output> {
    Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Fixture Author")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture Author")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .output()
        .ok()
}

/// Builds a fixture that is a real Git repository with one commit.
///
/// Returns `None` when Git is unavailable or any step fails.
pub fn git_fixture() -> Option<Fixture> {
    let fixture = Fixture::typical();
    let steps: [&[&str]; 3] = [
        &["init", "--initial-branch=main"],
        &["add", "."],
        &["commit", "-m", "fixture commit", "--no-gpg-sign"],
    ];

    for step in steps {
        let output = git(&fixture.root, step)?;
        if !output.status.success() {
            return None;
        }
    }

    Some(fixture)
}
