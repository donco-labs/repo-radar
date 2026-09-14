use std::env;
use std::path::PathBuf;

use repo_radar::{ScanConfig, render, scan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
    Html,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    root: PathBuf,
    top: usize,
    format: OutputFormat,
    count_lines: bool,
    read_git: bool,
    since_days: u32,
    read_cargo: bool,
    read_profile: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            top: 10,
            format: OutputFormat::Text,
            count_lines: true,
            read_git: true,
            since_days: 30,
            read_cargo: true,
            read_profile: true,
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

    let config = ScanConfig {
        count_lines: options.count_lines,
        read_git: options.read_git,
        activity_window_days: options.since_days,
        read_cargo: options.read_cargo,
        read_profile: options.read_profile,
        ..ScanConfig::default()
    };
    match scan(&options.root, &config) {
        Ok(mut report) => {
            report.largest_files.truncate(options.top);
            report.largest_directories.truncate(options.top);

            let mut output = String::new();
            let result = match options.format {
                OutputFormat::Text => {
                    render::text::write_summary(&mut output, &options.root, &report)
                }
                OutputFormat::Json => render::json::write_json(&mut output, &options.root, &report),
                OutputFormat::Html => render::html::write_html(&mut output, &options.root, &report),
            };
            result.expect("writing to a String cannot fail");
            print!("{output}");
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
                    "html" => OutputFormat::Html,
                    other => {
                        return Err(format!(
                            "unsupported format '{other}', expected text, json, or html"
                        ));
                    }
                };
                index += 2;
            }
            "--no-lines" => {
                options.count_lines = false;
                index += 1;
            }
            "--no-git" => {
                options.read_git = false;
                index += 1;
            }
            "--no-cargo" => {
                options.read_cargo = false;
                index += 1;
            }
            "--no-profile" => {
                options.read_profile = false;
                index += 1;
            }
            "--since-days" => {
                let value = take_value(arguments, index, "--since-days")?;
                options.since_days = value.parse().map_err(|_| {
                    format!("invalid value '{value}' for --since-days, expected a number")
                })?;
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
    --format text|json|html
                                                Output format (default: text; html is a standalone dashboard)
  --top N               Number of largest files to list (default: 10)
  --no-lines            Skip line counting (faster; lines report as not evaluated)
  --no-git              Skip the Git analyses (status and recent activity report as not evaluated)
  --since-days N        Days back the commit activity window covers (default: 30)
  --no-cargo            Skip reading Cargo.toml and Cargo.lock (report as not evaluated)
  --no-profile          Skip the project profile (stated purpose reports as not evaluated)
  -h, --help            Print this help and exit

Exit status:
  0  success
  1  the path is missing or is not a directory
  2  invalid usage",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(options.count_lines);
        assert!(options.read_git);
        assert_eq!(options.since_days, 30);
        assert!(options.read_cargo);
        assert!(options.read_profile);
    }

    #[test]
    fn no_lines_flag_disables_line_counting() {
        let options = parse_arguments(&arguments(&["--no-lines"])).unwrap();

        assert!(!options.count_lines);
    }

    #[test]
    fn no_cargo_flag_disables_cargo_analyses() {
        let options = parse_arguments(&arguments(&["--no-cargo"])).unwrap();

        assert!(!options.read_cargo);
    }

    #[test]
    fn no_git_flag_disables_git_analyses() {
        let options = parse_arguments(&arguments(&["--no-git"])).unwrap();

        assert!(!options.read_git);
    }

    #[test]
    fn no_profile_flag_disables_the_project_profile() {
        let options = parse_arguments(&arguments(&["--no-profile"])).unwrap();

        assert!(!options.read_profile);
    }

    #[test]
    fn since_days_flag_sets_the_activity_window() {
        let options = parse_arguments(&arguments(&["--since-days", "7"])).unwrap();

        assert_eq!(options.since_days, 7);
    }

    #[test]
    fn since_days_zero_is_legal_and_means_today_only() {
        let options = parse_arguments(&arguments(&["--since-days", "0"])).unwrap();

        assert_eq!(options.since_days, 0);
    }

    #[test]
    fn since_days_rejects_a_non_numeric_value() {
        assert!(parse_arguments(&arguments(&["--since-days", "many"])).is_err());
        assert!(parse_arguments(&arguments(&["--since-days"])).is_err());
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
