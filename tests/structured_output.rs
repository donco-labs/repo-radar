use std::process::Command;

use serde_json::Value;

mod common;

use common::{git_fixture_empty, git_fixture_with_changes, run};

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
