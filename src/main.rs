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
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            top: 10,
            format: OutputFormat::Text,
            count_lines: true,
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
    }

    #[test]
    fn no_lines_flag_disables_line_counting() {
        let options = parse_arguments(&arguments(&["--no-lines"])).unwrap();

        assert!(!options.count_lines);
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
