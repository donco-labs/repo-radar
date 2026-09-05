//! Enforcement of `docs/specs/000-safety-invariants.md`.
//!
//! Each test names the invariant it holds. These are the tests that let a user
//! point Repo Radar at an untrusted clone without reading it first.

mod common;

use std::fs;
use std::path::Path;

use common::{Fixture, TreeDigest, assert_target_unchanged, git, git_fixture, run};

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
        owned(&["--help"]),
        owned(&[root, "--format", "yaml"]),
        owned(&[root, "--top", "not-a-number"]),
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

#[test]
fn i2_git_state_is_never_mutated() {
    let Some(fixture) = git_fixture() else {
        eprintln!("skipping: git is unavailable");
        return;
    };
    let root = fixture.root.display().to_string();
    let git_dir = fixture.path(".git");

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
