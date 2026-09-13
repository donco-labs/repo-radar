use std::process::Command;

use serde_json::Value;

mod common;

use common::{Fixture, assert_target_unchanged, git_fixture_empty, git_fixture_with_changes, run};

#[test]
fn json_output_is_machine_readable_and_honors_top_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_repo-radar"))
        .args([".", "--format", "json", "--top", "0"])
        .output()
        .expect("repo-radar should run");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(report["version"], 1);
    assert!(report["repository"].is_string());
    assert!(report["files"].is_number());
    assert!(report["bytes"].is_number());
    assert!(report["by_extension"].is_object());
    assert_eq!(report["largest_files"].as_array().unwrap().len(), 0);
    assert!(report["warnings"].is_array());
}

#[test]
fn json_carries_language_and_directory_fields() {
    let output = Command::new(env!("CARGO_BIN_EXE_repo-radar"))
        .args([".", "--format", "json"])
        .output()
        .expect("repo-radar should run");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        report["version"], 1,
        "additive fields must not bump the version"
    );
    assert!(report["language_table_version"].is_number());
    assert!(report["by_language"].is_array());
    assert!(
        !report["by_language"].as_array().unwrap().is_empty(),
        "scanning this crate should surface at least one language"
    );
    assert!(report["largest_directories"].is_array());
    assert!(
        !report["largest_directories"].as_array().unwrap().is_empty(),
        "scanning this crate should surface at least one directory"
    );
    assert!(report["lines"].is_object());
    assert!(report["lines"]["evaluated"].is_boolean());
    assert!(report["lines"]["lines"].is_number());
}

#[test]
fn no_lines_flag_reports_not_evaluated_in_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_repo-radar"))
        .args([".", "--format", "json", "--no-lines"])
        .output()
        .expect("repo-radar should run");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        report["lines"]["evaluated"], false,
        "a disabled analysis must say so, not report a plausible-looking zero"
    );
    assert_eq!(report["lines"]["lines"], 0);
}

#[test]
fn html_output_is_a_self_contained_dashboard() {
    let output = Command::new(env!("CARGO_BIN_EXE_repo-radar"))
        .args([".", "--format", "html", "--top", "2"])
        .output()
        .expect("repo-radar should run");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let html = String::from_utf8(output.stdout).expect("HTML should be UTF-8");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("Repo Radar"));
    assert!(html.contains("no external assets or requests"));
    assert!(!html.contains("<script"));
    assert!(!html.contains("https://"));
}

#[test]
fn git_analyses_report_counts_in_json() {
    let Some(fixture) = git_fixture_with_changes() else {
        eprintln!("skipping: git is unavailable");
        return;
    };
    let root = fixture.root.display().to_string();

    let output = run(&[&root, "--format", "json"]);
    assert!(output.status.success());

    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(report["git_status"]["evaluated"], true);
    assert_eq!(report["git_status"]["modified"], 1);
    assert_eq!(report["git_status"]["staged"], 0);
    assert_eq!(report["git_status"]["untracked"], 1);
    assert_eq!(report["git_status"]["ignored"], 1);
}

#[test]
fn no_git_flag_reports_not_evaluated() {
    let Some(fixture) = git_fixture_with_changes() else {
        eprintln!("skipping: git is unavailable");
        return;
    };
    let root = fixture.root.display().to_string();

    let output = run(&[&root, "--format", "json", "--no-git"]);
    assert!(output.status.success());

    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    for analysis in ["git_status", "git_activity"] {
        assert_eq!(
            report[analysis]["evaluated"], false,
            "{analysis} must say it was not evaluated"
        );
        assert_eq!(report[analysis]["reason"], "disabled");
    }
    assert_eq!(report["git_status"]["modified"], 0);
    assert_eq!(report["git_status"]["staged"], 0);
    assert_eq!(report["git_status"]["untracked"], 0);
    assert_eq!(report["git_status"]["ignored"], 0);
    assert_eq!(report["git_activity"]["commits"], 0);
    assert_eq!(
        report["git_activity"]["by_day"].as_array().unwrap().len(),
        0
    );
}

#[test]
fn empty_repository_reports_status_but_not_activity() {
    let Some(fixture) = git_fixture_empty() else {
        eprintln!("skipping: git is unavailable");
        return;
    };
    let root = fixture.root.display().to_string();

    let output = run(&[&root, "--format", "json"]);
    assert!(output.status.success());

    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        report["git_status"]["evaluated"], true,
        "status must run even before the first commit"
    );
    assert_eq!(report["git_activity"]["evaluated"], false);
    assert_eq!(report["git_activity"]["reason"], "input_unavailable");
}

/// Spec 003, AC 5: direct dependencies and the locked package set are
/// reported separately, so a consumer can tell "declared" apart from
/// "resolved".
#[test]
fn cargo_analyses_report_dependencies_in_json() {
    let fixture = Fixture::typical();
    fixture.file(
        "Cargo.toml",
        b"[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n\n\
          [dependencies]\n\
          serde = \"1\"\n\
          log = { version = \"0.4\", features = [\"std\"] }\n",
    );
    fixture.file(
        "Cargo.lock",
        b"version = 4\n\n\
          [[package]]\n\
          name = \"serde\"\n\
          version = \"1.0.0\"\n\
          source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n\
          [[package]]\n\
          name = \"fixture\"\n\
          version = \"0.1.0\"\n",
    );
    let root = fixture.root.display().to_string();

    let report: Value = assert_target_unchanged(&fixture.root, "cargo analyses in JSON", || {
        let output = run(&[&root, "--format", "json"]);
        assert!(output.status.success());
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
    });

    assert_eq!(report["cargo_manifest"]["evaluated"], true);
    assert_eq!(report["cargo_manifest"]["package_name"], "fixture");
    assert_eq!(report["cargo_manifest"]["package_version"], "0.1.0");
    assert_eq!(
        report["cargo_manifest"]["dependencies"]
            .as_array()
            .expect("dependencies should be an array")
            .len(),
        2
    );

    assert_eq!(report["cargo_lock"]["evaluated"], true);
    assert_eq!(report["cargo_lock"]["lock_version"], 4);
    assert_eq!(report["cargo_lock"]["packages"], 2);
    assert_eq!(
        report["cargo_lock"]["local_packages"], 1,
        "the fixture package itself has no source and is local"
    );
}

/// Spec 003, AC 7: `--no-cargo` disables both Cargo analyses, and a disabled
/// analysis reports `not evaluated`, never a zero dressed up as data.
#[test]
fn no_cargo_flag_reports_not_evaluated() {
    let fixture = Fixture::typical();
    let root = fixture.root.display().to_string();

    let report: Value = assert_target_unchanged(&fixture.root, "--no-cargo", || {
        let output = run(&[&root, "--format", "json", "--no-cargo"]);
        assert!(output.status.success());
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
    });

    for analysis in ["cargo_manifest", "cargo_lock"] {
        assert_eq!(
            report[analysis]["evaluated"], false,
            "{analysis} must say it was not evaluated"
        );
        assert_eq!(report[analysis]["reason"], "disabled");
    }
    assert_eq!(report["cargo_manifest"]["package_name"], Value::Null);
    assert_eq!(
        report["cargo_manifest"]["dependencies"]
            .as_array()
            .expect("dependencies should be an array")
            .len(),
        0
    );
    assert_eq!(report["cargo_lock"]["packages"], 0);
    assert_eq!(report["cargo_lock"]["local_packages"], 0);
}

/// A directory that is not a Cargo project at all — no `Cargo.toml`, no
/// `Cargo.lock` — must still exit 0 with a complete report (spec 003, the
/// 4c parcel row; mirrors `non_git_directory_still_produces_a_report` in
/// `tests/safety_invariants.rs` for the Git analyses).
#[test]
fn non_cargo_directory_still_produces_a_report() {
    let fixture = Fixture::new();
    fixture.file("README.md", b"# no cargo here\n");
    let root = fixture.root.display().to_string();

    let report: Value = assert_target_unchanged(&fixture.root, "non-Cargo directory", || {
        let output = run(&[&root, "--format", "json"]);
        assert!(
            output.status.success(),
            "a non-Cargo directory must still exit 0 with a complete report"
        );
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
    });

    assert_eq!(report["cargo_manifest"]["evaluated"], false);
    assert_eq!(report["cargo_manifest"]["reason"], "input_unavailable");
    assert_eq!(report["cargo_lock"]["evaluated"], false);
    assert_eq!(report["cargo_lock"]["reason"], "input_unavailable");
}
