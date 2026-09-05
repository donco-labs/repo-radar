use std::process::Command;

fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_repo-radar"))
        .args(arguments)
        .output()
        .expect("repo-radar should run")
}

#[test]
fn help_names_every_supported_flag() {
    let output = run(&["--help"]);

    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("help should be UTF-8");
    for flag in ["--format", "--top", "--help"] {
        assert!(help.contains(flag), "help should document {flag}");
    }
}

#[test]
fn missing_directory_exits_with_status_one() {
    let output = run(&["definitely-not-a-directory"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.is_empty());
}

#[test]
fn invalid_usage_exits_with_status_two() {
    for arguments in [
        vec!["--top", "many"],
        vec!["--top"],
        vec!["--format", "yaml"],
        vec!["--unknown"],
        vec![".", "extra"],
    ] {
        let output = run(&arguments);
        assert_eq!(
            output.status.code(),
            Some(2),
            "expected usage error for {arguments:?}"
        );
    }
}
