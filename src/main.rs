use std::env;
use std::path::{Path, PathBuf};

use repo_radar::{ScanConfig, ScanReport, display_path, sanitize_for_terminal, scan};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: PathBuf,
    top: usize,
    format: OutputFormat,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            top: 10,
            format: OutputFormat::Text,
        }
    }
}

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();

    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_help();
        return;
    }

    let options = match parse_arguments(&arguments) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("repo-radar: {error}");
            eprintln!("Run `repo-radar --help` for usage.");
            std::process::exit(2);
        }
    };

    match scan(&options.root, &ScanConfig::default()) {
        Ok(mut report) => {
            report.largest_files.truncate(options.top);
            match options.format {
                OutputFormat::Text => print_summary(&options.root, &report),
                OutputFormat::Json => print_json(&options.root, &report),
            }
        }
        Err(error) => {
            eprintln!("repo-radar: {error}");
            std::process::exit(1);
        }
    }
}

fn parse_arguments(arguments: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut root_set = false;
    let mut index = 0;

    while index < arguments.len() {
        let argument = arguments[index].as_str();
        match argument {
            "--top" => {
                let value = take_value(arguments, index, "--top")?;
                options.top = value
                    .parse()
                    .map_err(|_| format!("invalid value '{value}' for --top, expected a number"))?;
                index += 2;
            }
            "--format" => {
                let value = take_value(arguments, index, "--format")?;
                options.format = match value {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    other => {
                        return Err(format!(
                            "unsupported format '{other}', expected text or json"
                        ));
                    }
                };
                index += 2;
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown flag '{other}'"));
            }
            other => {
                if root_set {
                    return Err(format!("unexpected argument '{other}', expected one path"));
                }
                options.root = PathBuf::from(other);
                root_set = true;
                index += 1;
            }
        }
    }

    Ok(options)
}

fn take_value<'a>(arguments: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    arguments
        .get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn print_help() {
    println!(
        "repo-radar {}

Summarize files in a local repository.

Usage:
  repo-radar [PATH] [OPTIONS]

Arguments:
  PATH                  Directory to scan (default: the current directory)

Options:
  --format text|json    Output format (default: text)
  --top N               Number of largest files to list (default: 10)
  -h, --help            Print this help and exit

Exit status:
  0  success
  1  the path is missing or is not a directory
  2  invalid usage",
        env!("CARGO_PKG_VERSION")
    );
}

fn print_summary(root: &Path, summary: &ScanReport) {
    println!("Repository: {}", display_path(root));
    println!("Files:      {}", summary.files);
    println!("Size:       {}", format_bytes(summary.bytes));

    println!("\nLanguages / extensions:");
    for (extension, count) in &summary.by_extension {
        println!("  {:<16} {count}", sanitize_for_terminal(extension));
    }

    println!("\nLargest files:");
    for file in &summary.largest_files {
        println!(
            "  {:>10}  {}",
            format_bytes(file.bytes),
            display_path(&file.path)
        );
    }
}

#[derive(Serialize)]
struct JsonReport<'a> {
    version: u8,
    repository: &'a Path,
    files: usize,
    bytes: u64,
    by_extension: &'a std::collections::BTreeMap<String, usize>,
    largest_files: &'a [repo_radar::FileEntry],
    warnings: &'a [repo_radar::ScanWarning],
}

fn print_json(root: &Path, report: &ScanReport) {
    let output = JsonReport {
        version: 1,
        repository: root,
        files: report.files,
        bytes: report.bytes,
        by_extension: &report.by_extension,
        largest_files: &report.largest_files,
        warnings: &report.warnings,
    };
    println!(
        "{}",
        serde_json::to_string(&output).expect("JSON report should serialize")
    );
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_for_humans() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KiB");
    }

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn defaults_to_current_directory_text_and_top_ten() {
        let options = parse_arguments(&[]).unwrap();

        assert_eq!(options, Options::default());
        assert_eq!(options.root, PathBuf::from("."));
        assert_eq!(options.top, 10);
        assert_eq!(options.format, OutputFormat::Text);
    }

    #[test]
    fn parses_path_and_flags_in_any_order() {
        let options = parse_arguments(&arguments(&["--format", "json", "src", "--top", "3"]))
            .expect("flags should parse before and after the path");

        assert_eq!(options.root, PathBuf::from("src"));
        assert_eq!(options.top, 3);
        assert_eq!(options.format, OutputFormat::Json);
    }

    #[test]
    fn rejects_invalid_usage_instead_of_falling_back() {
        assert!(parse_arguments(&arguments(&["--top", "many"])).is_err());
        assert!(parse_arguments(&arguments(&["--top"])).is_err());
        assert!(parse_arguments(&arguments(&["--format", "yaml"])).is_err());
        assert!(parse_arguments(&arguments(&["--format"])).is_err());
        assert!(parse_arguments(&arguments(&["--tpo", "5"])).is_err());
        assert!(parse_arguments(&arguments(&["one", "two"])).is_err());
    }
}
