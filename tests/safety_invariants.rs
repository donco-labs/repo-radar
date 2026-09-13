//! Enforcement of `docs/specs/000-safety-invariants.md`.
//!
//! Each test names the invariant it holds. These are the tests that let a user
//! point Repo Radar at an untrusted clone without reading it first.

mod common;

use std::fs;
use std::path::Path;

use common::{
    Fixture, TreeDigest, assert_target_unchanged, cargo_fixture_hostile_dependency_name,
    cargo_fixture_malformed, git, git_fixture, run,
};

#[cfg(unix)]
use common::git_fixture_hostile_fsmonitor;

/// Every invocation a user could plausibly make, including invalid ones.
///
/// Failure paths are covered deliberately: a command that cleans up after
/// itself on success but not on error still violates I1.
fn every_invocation(root: &str) -> Vec<Vec<String>> {
    let owned = |arguments: &[&str]| -> Vec<String> {
        arguments.iter().map(|value| (*value).to_owned()).collect()
    };

    vec![
        owned(&[root]),
        owned(&[root, "--format", "text"]),
        owned(&[root, "--format", "json"]),
        owned(&[root, "--top", "0"]),
        owned(&[root, "--top", "1000000"]),
        // Line counting opens and reads every file in the target, so a
        // default run and a `--no-lines` run are the change most likely to
        // touch access times or otherwise disturb the tree (spec 003, AC7).
        owned(&[root, "--no-lines"]),
        // The Git analyses run by default; this parcel's own commands.
        owned(&[root, "--no-git"]),
        owned(&[root, "--since-days", "0"]),
        owned(&[root, "--since-days", "365"]),
        owned(&["--help"]),
        owned(&[root, "--format", "yaml"]),
        owned(&[root, "--top", "not-a-number"]),
        owned(&[root, "--since-days", "not-a-number"]),
        owned(&[root, "--unknown-flag"]),
        owned(&[root, "extra-path"]),
        owned(&["definitely-not-a-directory"]),
    ]
}

#[test]
fn i1_no_command_modifies_the_inspected_repository() {
    let fixture = Fixture::typical();
    let root = fixture.root.display().to_string();

    for arguments in every_invocation(&root) {
        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        assert_target_unchanged(&fixture.root, &format!("repo-radar {arguments:?}"), || {
            run(&borrowed);
        });
    }
}

#[test]
fn i1_holds_for_a_read_only_directory() {
    let fixture = Fixture::typical();
    let root = fixture.root.display().to_string();

    assert_target_unchanged(
        &fixture.root,
        "scan of a tree containing an unreadable path",
        || {
            let restricted = fixture.path("restricted");
            fs::create_dir(&restricted).expect("restricted directory should be created");
            fs::write(restricted.join("hidden.rs"), b"secret").expect("file should be written");

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000))
                    .expect("permissions should be set");
            }

            let output = run(&[&root]);

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&restricted, fs::Permissions::from_mode(0o755))
                    .expect("permissions should be restored");
            }

            assert!(
                output.status.success(),
                "an unreadable entry must warn, not abort"
            );
            fs::remove_dir_all(&restricted).expect("restricted directory should be removed");
        },
    );
}

/// Rewrites a tracked file with identical content but a newer mtime: a
/// stale index entry, the shape that makes `git status` do real comparison
/// work against the working tree rather than trusting cached stat data.
/// Every invocation in [`every_invocation`] runs with the Git analyses
/// enabled by default, so this is what gives `i2_git_state_is_never_mutated`
/// something to fail on if `--no-optional-locks` is ever dropped.
fn make_index_stale(fixture: &Fixture) {
    let path = fixture.path("src/main.rs");
    let contents = fs::read(&path).expect("fixture file should be readable");
    std::thread::sleep(std::time::Duration::from_millis(50));
    fs::write(&path, &contents).expect("fixture file should be rewritable with identical bytes");
}

#[test]
fn i2_git_state_is_never_mutated() {
    let Some(fixture) = git_fixture() else {
        eprintln!("skipping: git is unavailable");
        return;
    };
    let root = fixture.root.display().to_string();
    let git_dir = fixture.path(".git");

    // Every invocation below runs with the Git analyses enabled (the
    // default): this is the test parcel 4b's `analyze` must pass under.
    make_index_stale(&fixture);

    for arguments in every_invocation(&root) {
        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        let before = TreeDigest::capture(&git_dir);

        run(&borrowed);

        let after = TreeDigest::capture(&git_dir);
        if let Some(difference) = before.difference(&after) {
            panic!(
                "repo-radar {arguments:?} modified .git.\n\
                 Git state is read-only (spec 000, invariant I2).\n{difference}"
            );
        }
    }
}

#[test]
fn i2_head_and_index_survive_a_scan() {
    let Some(fixture) = git_fixture() else {
        eprintln!("skipping: git is unavailable");
        return;
    };
    let root = fixture.root.display().to_string();

    let head_before = git(&fixture.root, &["rev-parse", "HEAD"]).expect("git should run");
    let status_before = git(&fixture.root, &["status", "--porcelain"]).expect("git should run");

    run(&[&root, "--format", "json"]);

    let head_after = git(&fixture.root, &["rev-parse", "HEAD"]).expect("git should run");
    let status_after = git(&fixture.root, &["status", "--porcelain"]).expect("git should run");

    assert_eq!(head_before.stdout, head_after.stdout, "HEAD moved");
    assert_eq!(
        status_before.stdout, status_after.stdout,
        "working tree status changed"
    );
    assert!(
        status_after.stdout.is_empty(),
        "a scan must not dirty the working tree"
    );
}

/// I3: a hostile repository's own `.git/config` must not get to execute
/// code via `core.fsmonitor` during `git status`. This is the test that
/// matters most in this file: it is a verified, working exploit, not a
/// hypothetical one, and it is only blocked because every invocation in
/// `src/analysis/git.rs` carries `-c core.fsmonitor=false` ahead of the
/// subcommand, overriding the repository's own setting.
#[cfg(unix)]
#[test]
fn i3_hostile_fsmonitor_config_is_not_executed() {
    let canary = std::env::temp_dir().join(format!(
        "repo-radar-canary-i3-fsmonitor-{}",
        std::process::id()
    ));
    let _ = fs::remove_file(&canary);

    let Some(fixture) = git_fixture_hostile_fsmonitor(&canary) else {
        eprintln!("skipping: git is unavailable");
        return;
    };
    let root = fixture.root.display().to_string();

    assert_target_unchanged(
        &fixture.root,
        "scan of a hostile core.fsmonitor config",
        || {
            let output = run(&[&root, "--format", "json"]);
            assert!(
                output.status.success(),
                "the report must still render even though the config is hostile"
            );
        },
    );

    assert!(
        !canary.exists(),
        "the repository's own core.fsmonitor config executed a command during \
         a scan (spec 000, invariant I3)"
    );
    let _ = fs::remove_file(&canary);
}

/// I4: hostile repository content must not escape into the shell or the tree.
#[test]
fn i4_hostile_names_do_not_escape_the_root() {
    let fixture = Fixture::typical();
    let canary = fixture.root.parent().unwrap().join("repo-radar-canary-i4");
    let _ = fs::remove_file(&canary);

    let hostile = [
        "src/$(touch ../repo-radar-canary-i4).rs",
        "src/`touch ../repo-radar-canary-i4`.rs",
        "src/; touch ../repo-radar-canary-i4 ;.rs",
        "src/a|b&c.rs",
        "src/--format.rs",
        "src/-rf.rs",
        "src/'quoted'.rs",
        "src/\"double\".rs",
        "src/back\\slash.rs",
        "src/new\nline.rs",
    ];

    let mut created = 0;
    for name in hostile {
        if fs::write(fixture.root.join(name), b"x").is_ok() {
            created += 1;
        }
    }
    assert!(created > 0, "no hostile fixture names could be created");

    let root = fixture.root.display().to_string();
    assert_target_unchanged(&fixture.root, "scan of hostile file names", || {
        let output = run(&[&root, "--format", "json"]);
        assert!(
            output.status.success(),
            "hostile names must be data, not a failure"
        );
    });

    assert!(
        !canary.exists(),
        "a file name was interpreted as a shell command (spec 000, invariant I4)"
    );
    let _ = fs::remove_file(&canary);
}

#[test]
fn i4_control_sequences_do_not_reach_the_terminal() {
    let fixture = Fixture::typical();
    let hostile = "src/evil\u{1b}[31m\u{7}name.rs";

    if fs::write(fixture.root.join(hostile), b"x").is_err() {
        eprintln!("skipping: filesystem rejected the hostile name");
        return;
    }

    let output = run(&[&fixture.root.display().to_string()]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("name.rs"),
        "the file must still be reported"
    );
    for forbidden in ['\u{1b}', '\u{7}'] {
        assert!(
            !stdout.contains(forbidden),
            "control character {forbidden:?} reached stdout (spec 000, invariant I4)"
        );
    }
}

/// I4, for the Git parcel: the build sheet asked for a hostile *branch*
/// name, but Git's own ref validation rejects one — verified empirically,
/// `git init --initial-branch` containing an ESC byte exits 128 with
/// `fatal: invalid initial branch name`, so that fixture cannot exist. This
/// parcel also never reads branch names at all (out of scope until spec
/// 013), so there is no code path for one to reach output regardless. The
/// nearest real vector into this parcel's own behavior is an untracked file
/// name inside a Git-enabled scan: `GitStatus` counts it but never names it
/// (see the doc comment on `GitStatus`), and the rest of the report must
/// still hold I4 the same way it does outside a Git repository.
#[test]
fn i4_hostile_filename_in_a_git_repository_does_not_reach_output_unsanitized() {
    let Some(fixture) = git_fixture() else {
        eprintln!("skipping: git is unavailable");
        return;
    };
    let hostile = "src/evil\u{1b}[31m\u{7}name.rs";

    if fs::write(fixture.root.join(hostile), b"x").is_err() {
        eprintln!("skipping: filesystem rejected the hostile name");
        return;
    }

    let output = run(&[&fixture.root.display().to_string(), "--format", "text"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("name.rs"),
        "the file must still be reported"
    );
    for forbidden in ['\u{1b}', '\u{7}'] {
        assert!(
            !stdout.contains(forbidden),
            "control character {forbidden:?} reached stdout from a Git-enabled scan \
             (spec 000, invariant I4)"
        );
    }
}

#[test]
fn i6_the_default_run_declares_no_network_capable_dependency() {
    let lockfile = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock"))
        .expect("Cargo.lock should be readable");

    // Dependency-level enforcement of the offline default. This cannot observe
    // a syscall, so it guards the property that actually makes a syscall
    // possible: linking something that can open a socket. When spec 017 adds
    // `--online`, its crate is expected here and this list gains an allowance
    // alongside a syscall-level test.
    let networking = [
        "reqwest",
        "hyper",
        "curl",
        "ureq",
        "surf",
        "isahc",
        "attohttpc",
        "socket2",
        "mio",
        "tokio",
        "async-std",
        "smol",
        "rustls",
        "native-tls",
        "openssl",
        "trust-dns",
    ];

    let mut found = Vec::new();
    for line in lockfile.lines() {
        if let Some(name) = line
            .strip_prefix("name = \"")
            .and_then(|r| r.strip_suffix('"'))
            && networking.contains(&name)
        {
            found.push(name.to_owned());
        }
    }

    assert!(
        found.is_empty(),
        "network-capable dependencies present: {found:?}.\n\
         Repo Radar is offline by default (spec 000, invariant I6). Adding one \
         requires an explicit opt-in flag and an update to this test."
    );
}

#[test]
fn i8_symlinks_are_not_followed_out_of_the_root() {
    let fixture = Fixture::typical();
    let outside = Fixture::new();
    outside.file("secret.rs", b"content outside the scanned root\n");

    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(&outside.root, fixture.path("escape")).is_err() {
            eprintln!("skipping: symlinks unavailable");
            return;
        }
    }
    #[cfg(not(unix))]
    {
        eprintln!("skipping: symlink test is unix-only");
        return;
    }

    let output = run(&[&fixture.root.display().to_string(), "--format", "json"]);
    assert!(output.status.success());

    let report = String::from_utf8_lossy(&output.stdout);
    assert!(
        !report.contains("secret.rs"),
        "traversal followed a symlink out of the root (spec 000, invariant I8)"
    );
}

/// Spec 003, acceptance criterion 4: a non-Git directory still produces a
/// successful, complete report. Every Git failure degrades to
/// `NotEvaluated` rather than aborting the scan.
#[test]
fn non_git_directory_still_produces_a_report() {
    let fixture = Fixture::typical();
    let root = fixture.root.display().to_string();

    let output = run(&[&root, "--format", "json"]);

    assert!(
        output.status.success(),
        "a non-Git directory must still exit 0 with a complete report (spec 003, AC 4)"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(report["git_status"]["evaluated"], false);
    assert_eq!(report["git_status"]["reason"], "input_unavailable");
    assert_eq!(report["git_activity"]["evaluated"], false);
    assert_eq!(report["git_activity"]["reason"], "input_unavailable");
}

/// Spec 003, AC 6: a malformed manifest must not abort the scan; the
/// manifest analysis reports its own failure and the rest of the report
/// stays complete.
#[test]
fn malformed_manifest_does_not_abort_the_scan() {
    let fixture = cargo_fixture_malformed();
    let root = fixture.root.display().to_string();

    let report: serde_json::Value =
        assert_target_unchanged(&fixture.root, "scan of a malformed Cargo.toml", || {
            let output = run(&[&root, "--format", "json"]);
            assert!(
                output.status.success(),
                "a malformed manifest must not abort the scan (spec 003, AC 6)"
            );
            serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
        });

    assert_eq!(report["cargo_manifest"]["evaluated"], false);
    assert_eq!(report["cargo_manifest"]["reason"], "failed");
    assert!(
        report["files"].as_u64().expect("files should be a number") > 0,
        "the rest of the report must still be complete"
    );
}

/// I4: a dependency name is untrusted manifest content, the same way a file
/// name is. The text renderer must neutralize its control characters before
/// the name reaches the terminal.
#[test]
fn i4_hostile_dependency_name_does_not_reach_output_unsanitized() {
    let fixture = cargo_fixture_hostile_dependency_name();
    let root = fixture.root.display().to_string();

    let stdout =
        assert_target_unchanged(&fixture.root, "scan of a hostile dependency name", || {
            let output = run(&[&root, "--format", "text"]);
            assert!(output.status.success());
            String::from_utf8_lossy(&output.stdout).into_owned()
        });

    assert!(
        stdout.contains("name"),
        "the dependency must still be reported: {stdout:?}"
    );
    assert!(
        !stdout.contains('\u{1b}'),
        "a control character from a dependency name reached stdout (spec 000, invariant I4)"
    );
}

#[test]
fn i9_hostile_input_degrades_instead_of_crashing() {
    let fixture = Fixture::new();

    let mut deep = fixture.root.clone();
    for level in 0..80 {
        deep = deep.join(format!("level-{level}"));
    }
    fs::create_dir_all(&deep).expect("deep tree should be created");
    fs::write(deep.join("leaf.rs"), b"deep").expect("leaf should be written");

    fixture.file("invalid-utf8.rs", &[0xff, 0xfe, 0x00, 0x01, 0x80]);
    fixture.file("binary.bin", &[0u8; 4096]);
    fixture.file("Cargo.toml", b"[package\nthis is not valid toml");

    let output = run(&[&fixture.root.display().to_string(), "--format", "json"]);

    assert!(
        output.status.success(),
        "hostile input must degrade, not fail (spec 000, invariant I9)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "a panic reached the user: {stderr}"
    );
    serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("output must remain valid JSON under hostile input");
}

#[test]
fn i10_an_unreadable_entry_is_reported_rather_than_ignored() {
    let fixture = Fixture::typical();
    let restricted = fixture.path("restricted");
    fs::create_dir(&restricted).expect("directory should be created");
    fs::write(restricted.join("file.rs"), b"x").expect("file should be written");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000)).is_err() {
            eprintln!("skipping: permissions unavailable");
            return;
        }
        if fs::read_dir(&restricted).is_ok() {
            fs::set_permissions(&restricted, fs::Permissions::from_mode(0o755)).ok();
            eprintln!("skipping: running with privileges that ignore permissions");
            return;
        }
    }
    #[cfg(not(unix))]
    {
        eprintln!("skipping: permission test is unix-only");
        return;
    }

    let output = run(&[&fixture.root.display().to_string(), "--format", "json"]);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o755)).ok();
    }

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("output should be JSON");
    let warnings = report["warnings"].as_array().expect("warnings array");

    assert!(
        !warnings.is_empty(),
        "an unreadable directory was silently skipped rather than reported \
         (spec 000, invariant I10)"
    );
}

/// A named mutation the harness is expected to notice.
type Mutation<'a> = (&'static str, Box<dyn Fn() + 'a>);

/// The harness must be able to fail, or every test using it proves nothing.
#[test]
fn the_harness_detects_each_kind_of_mutation() {
    let fixture = Fixture::typical();

    let baseline = TreeDigest::capture(&fixture.root);
    assert!(
        baseline
            .difference(&TreeDigest::capture(&fixture.root))
            .is_none(),
        "an unchanged tree must compare equal"
    );

    let mutations: Vec<Mutation<'_>> = vec![
        (
            "created file",
            Box::new(|| {
                fs::write(fixture.path("intruder.rs"), b"new").ok();
            }),
        ),
        (
            "modified content",
            Box::new(|| {
                fs::write(fixture.path("src/main.rs"), b"fn main() { /* edited */ }").ok();
            }),
        ),
        (
            "removed file",
            Box::new(|| {
                fs::remove_file(fixture.path("README.md")).ok();
            }),
        ),
        (
            "created directory",
            Box::new(|| {
                fs::create_dir_all(fixture.path("new-directory")).ok();
            }),
        ),
    ];

    for (description, mutate) in mutations {
        let before = TreeDigest::capture(&fixture.root);
        mutate();
        let after = TreeDigest::capture(&fixture.root);
        assert!(
            before.difference(&after).is_some(),
            "the harness failed to detect a {description}, so every immutability \
             assertion built on it is worthless"
        );
    }
}

#[test]
fn the_harness_rejects_an_empty_fixture() {
    let empty = Fixture::new();
    let result = std::panic::catch_unwind(|| {
        assert_target_unchanged(&empty.root, "no-op over an empty tree", || {});
    });

    assert!(
        result.is_err(),
        "asserting immutability over an empty tree proves nothing and must fail loudly"
    );
}

#[test]
fn i5_writes_land_outside_the_target() {
    // Repo Radar currently writes nothing at all. This test pins that fact so
    // the first feature to add an output path (spec 011 cache, spec 019 report)
    // must extend it rather than quietly gaining a write.
    let fixture = Fixture::typical();
    let root = fixture.root.display().to_string();

    assert_target_unchanged(&fixture.root, "every current command", || {
        for arguments in every_invocation(&root) {
            let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
            run(&borrowed);
        }
    });
}

/// Spec 003, AC 3: Git analysis never shells out with user-controlled
/// arguments, and repository paths are passed safely.
///
/// The repository root is attacker-influenced in practice — a user clones
/// whatever name a hostile project chose. This scans a Git repository whose
/// directory name carries shell metacharacters and a command substitution
/// that would create a canary outside the root. `Command` passes an argv
/// straight to `execve` with no shell, and the root reaches Git through
/// `current_dir` rather than an argv element, so the name stays inert data.
///
/// The Git analysis is asserted to have actually run: a test where Git
/// silently failed would pass for the wrong reason and prove nothing.
#[test]
fn i4_shell_metacharacters_in_the_repository_path_are_inert() {
    let fixture = Fixture::new();
    let canary = fixture.root.join("CANARY");

    let hostile = fixture
        .root
        .join("repo $(touch ../CANARY) `touch ../CANARY` ; touch ../CANARY");
    if fs::create_dir_all(&hostile).is_err() {
        eprintln!("skipping: the filesystem rejected the hostile directory name");
        return;
    }
    fixture.file("repo-marker.rs", b"fn main() {}\n");
    fs::write(hostile.join("src.rs"), b"pub fn work() {}\n")
        .expect("fixture file should be written");

    let steps: [&[&str]; 3] = [
        &["init", "--initial-branch=main"],
        &["add", "."],
        &["commit", "-m", "hostile path fixture", "--no-gpg-sign"],
    ];
    for step in steps {
        let Some(output) = git(&hostile, step) else {
            eprintln!("skipping: git is unavailable");
            return;
        };
        if !output.status.success() {
            eprintln!("skipping: git could not initialize the hostile-path fixture");
            return;
        }
    }

    let root = hostile.display().to_string();
    let output = run(&[&root, "--format", "json"]);

    assert!(
        output.status.success(),
        "a repository whose path contains shell metacharacters must still scan"
    );
    assert!(
        !canary.exists(),
        "a repository path was interpreted by a shell (spec 000, invariant I4; spec 003, AC 3)"
    );

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        report["git_status"]["evaluated"], true,
        "the Git analysis must actually have run, or this test proves nothing"
    );
}
