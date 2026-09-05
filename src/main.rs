use std::env;
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};

use repo_radar::{ScanConfig, ScanReport, display_path, sanitize_for_terminal, scan};
use serde::Serialize;

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
            match options.format {
                OutputFormat::Text => print_summary(&options.root, &report),
                OutputFormat::Json => print_json(&options.root, &report),
                OutputFormat::Html => print_html(&options.root, &report),
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

fn print_summary(root: &Path, summary: &ScanReport) {
    println!("Repository: {}", display_path(root));
    println!("Files:      {}", summary.files);
    println!("Size:       {}", format_bytes(summary.bytes));
    if summary.lines.evaluated {
        println!("Lines:      {}", summary.lines.lines);
    } else {
        println!("Lines:      not evaluated");
    }

    println!("\nLanguages:");
    for language in &summary.by_language {
        if summary.lines.evaluated {
            println!(
                "  {:<16} {:>3} files {:>10}  {:>10} lines",
                sanitize_for_terminal(&language.language),
                language.files,
                format_bytes(language.bytes),
                language.lines
            );
        } else {
            println!(
                "  {:<16} {:>3} files {:>10}",
                sanitize_for_terminal(&language.language),
                language.files,
                format_bytes(language.bytes)
            );
        }
    }

    println!("\nExtensions:");
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

    println!("\nLargest directories (aggregate, including subdirectories):");
    for directory in &summary.largest_directories {
        println!(
            "  {:>10}  {}",
            format_bytes(directory.bytes),
            display_path(&directory.path)
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
    language_table_version: u32,
    by_language: &'a [repo_radar::LanguageStat],
    largest_files: &'a [repo_radar::FileEntry],
    largest_directories: &'a [repo_radar::DirectoryEntry],
    lines: &'a repo_radar::LineCounts,
    warnings: &'a [repo_radar::ScanWarning],
}

fn print_json(root: &Path, report: &ScanReport) {
    let output = JsonReport {
        version: 1,
        repository: root,
        files: report.files,
        bytes: report.bytes,
        by_extension: &report.by_extension,
        language_table_version: repo_radar::LANGUAGE_TABLE_VERSION,
        by_language: &report.by_language,
        largest_files: &report.largest_files,
        largest_directories: &report.largest_directories,
        lines: &report.lines,
        warnings: &report.warnings,
    };
    println!(
        "{}",
        serde_json::to_string(&output).expect("JSON report should serialize")
    );
}

fn print_html(root: &Path, report: &ScanReport) {
    let mut html = String::new();
    let repository = escape_html(&display_path(root));
    let lines_stat = if report.lines.evaluated {
        report.lines.lines.to_string()
    } else {
        "not evaluated".to_owned()
    };
    let _ = write!(
        html,
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Repo Radar | {repository}</title>
<style>
:root {{
  color-scheme: light;
  --ink: #18252b;
  --muted: #66777b;
  --paper: #f4f1e9;
  --panel: #fffdf8;
  --line: #d8d3c6;
  --accent: #e36c3d;
  --accent-dark: #a94329;
  --teal: #257d78;
  --shadow: 0 18px 45px rgba(38, 49, 45, .09);
}}
* {{ box-sizing: border-box; }}
body {{ margin: 0; background: var(--paper); color: var(--ink); font: 16px/1.5 ui-sans-serif, system-ui, sans-serif; }}
main {{ width: min(1180px, calc(100% - 40px)); margin: 0 auto; padding: 42px 0 64px; }}
header {{ display: flex; justify-content: space-between; gap: 32px; align-items: end; margin-bottom: 34px; }}
.eyebrow {{ color: var(--accent-dark); font-size: .76rem; font-weight: 800; letter-spacing: .12em; text-transform: uppercase; }}
h1, h2, p {{ margin: 0; }}
h1 {{ margin-top: 8px; font: 700 clamp(2.4rem, 6vw, 5.5rem)/.95 Georgia, serif; letter-spacing: 0; }}
h2 {{ font-size: 1.1rem; margin-bottom: 18px; }}
.path {{ color: var(--muted); max-width: 38rem; overflow-wrap: anywhere; }}
.badge {{ border: 1px solid var(--line); border-radius: 999px; padding: 7px 12px; color: var(--teal); background: var(--panel); font-size: .78rem; font-weight: 700; white-space: nowrap; }}
.stats {{ display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-bottom: 14px; }}
.stat, .panel {{ background: var(--panel); border: 1px solid var(--line); box-shadow: var(--shadow); }}
.stat {{ padding: 22px; min-height: 122px; }}
.stat strong {{ display: block; font: 700 2rem/1.1 Georgia, serif; }}
.stat span {{ display: block; color: var(--muted); margin-top: 8px; font-size: .86rem; }}
.grid {{ display: grid; grid-template-columns: 1.05fr .95fr; gap: 14px; }}
.panel {{ padding: 24px; }}
.panel-wide {{ grid-column: 1 / -1; }}
.bar-row {{ display: grid; grid-template-columns: minmax(90px, 1fr) 2fr auto; gap: 12px; align-items: center; margin: 13px 0; font-size: .9rem; }}
.bar-label {{ overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
.bar-track {{ background: #e9e4d8; height: 9px; border-radius: 99px; overflow: hidden; }}
.bar {{ height: 100%; background: var(--teal); border-radius: inherit; min-width: 3px; }}
table {{ border-collapse: collapse; width: 100%; }}
th, td {{ border-bottom: 1px solid var(--line); padding: 11px 0; text-align: left; }}
th {{ color: var(--muted); font-size: .75rem; letter-spacing: .08em; text-transform: uppercase; }}
td:last-child, th:last-child {{ text-align: right; white-space: nowrap; }}
code {{ color: var(--accent-dark); font: .9em ui-monospace, SFMono-Regular, monospace; overflow-wrap: anywhere; }}
.empty {{ color: var(--muted); }}
.warning {{ color: var(--accent-dark); border-left: 3px solid var(--accent); padding-left: 12px; margin-top: 12px; }}
footer {{ color: var(--muted); font-size: .78rem; margin-top: 28px; }}
@media (max-width: 760px) {{ main {{ width: min(100% - 24px, 560px); padding-top: 26px; }} header {{ display: block; }} .badge {{ display: inline-block; margin-top: 18px; }} .stats, .grid {{ grid-template-columns: 1fr; }} .panel-wide {{ grid-column: auto; }} .bar-row {{ grid-template-columns: 100px 1fr auto; }} }}
</style>
</head>
<body>
<main>
<header><div><div class="eyebrow">Repository observatory</div><h1>Repo Radar</h1><p class="path">{repository}</p></div><div class="badge">READ-ONLY SCAN</div></header>
<section class="stats" aria-label="Repository totals">
<div class="stat"><strong>{}</strong><span>files inventoried</span></div>
<div class="stat"><strong>{}</strong><span>total size</span></div>
<div class="stat"><strong>{}</strong><span>extensions detected</span></div>
<div class="stat"><strong>{}</strong><span>lines counted</span></div>
</section>
<section class="grid">
<article class="panel"><h2>Composition</h2>"##,
        report.files,
        format_bytes(report.bytes),
        report.by_extension.len(),
        lines_stat
    );

    // Bar width reflects bytes, not file count, so a handful of huge files
    // are not visually dwarfed by many tiny ones.
    let maximum_language_bytes = report
        .by_language
        .iter()
        .map(|language| language.bytes)
        .max()
        .unwrap_or(1)
        .max(1);
    for language in &report.by_language {
        let percentage = (language.bytes as f64 / maximum_language_bytes as f64) * 100.0;
        let _ = write!(
            html,
            "<div class=\"bar-row\"><span class=\"bar-label\">{}</span><span class=\"bar-track\"><span class=\"bar\" style=\"width:{percentage:.1}%\"></span></span><strong>{}</strong></div>",
            escape_html(&language.language),
            format_bytes(language.bytes)
        );
    }
    if report.by_language.is_empty() {
        html.push_str("<p class=\"empty\">No files found.</p>");
    }

    html.push_str("</article><article class=\"panel\"><h2>Largest files</h2>");
    if report.largest_files.is_empty() {
        html.push_str("<p class=\"empty\">No files found.</p>");
    } else {
        html.push_str("<table><thead><tr><th>Path</th><th>Size</th></tr></thead><tbody>");
        for file in &report.largest_files {
            let _ = write!(
                html,
                "<tr><td><code>{}</code></td><td>{}</td></tr>",
                escape_html(&display_path(&file.path)),
                format_bytes(file.bytes)
            );
        }
        html.push_str("</tbody></table>");
    }
    html.push_str(
        "</article><article class=\"panel panel-wide\"><h2>Largest directories (aggregate, including subdirectories)</h2>",
    );
    if report.largest_directories.is_empty() {
        html.push_str("<p class=\"empty\">No files found.</p>");
    } else {
        html.push_str("<table><thead><tr><th>Path</th><th>Size</th></tr></thead><tbody>");
        for directory in &report.largest_directories {
            let _ = write!(
                html,
                "<tr><td><code>{}</code></td><td>{}</td></tr>",
                escape_html(&display_path(&directory.path)),
                format_bytes(directory.bytes)
            );
        }
        html.push_str("</tbody></table>");
    }
    html.push_str("</article><article class=\"panel panel-wide\"><h2>Scan notes</h2>");
    if report.warnings.is_empty() {
        html.push_str("<p class=\"empty\">No warnings. The scan completed cleanly.</p>");
    } else {
        for warning in &report.warnings {
            let _ = write!(
                html,
                "<p class=\"warning\"><code>{}</code>: {}</p>",
                escape_html(&display_path(&warning.path)),
                escape_html(&warning.message)
            );
        }
    }
    let _ = write!(
        html,
        "</article></section><footer>Generated by Repo Radar {}. This report contains no external assets or requests.</footer></main></body></html>",
        env!("CARGO_PKG_VERSION")
    );
    print!("{html}");
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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

    #[test]
    fn html_escapes_repository_content() {
        assert_eq!(escape_html("<script>\"&"), "&lt;script&gt;&quot;&amp;");
    }
}
