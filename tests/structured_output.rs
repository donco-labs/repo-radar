use std::process::Command;

use serde_json::Value;

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
