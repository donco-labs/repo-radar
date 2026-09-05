use std::env;
use std::path::{Path, PathBuf};

use repo_radar::{ScanConfig, ScanReport, scan};

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();

    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_help();
        return;
    }

    let root = arguments
        .first()
        .filter(|argument| !argument.starts_with('-'))
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let top = parse_top(&arguments);

    match scan(&root, &ScanConfig::default()) {
        Ok(mut report) => {
            report.largest_files.truncate(top);
            print_summary(&root, &report);
        }
        Err(error) => {
            eprintln!("repo-radar: {error}");
            std::process::exit(1);
        }
    }
}

fn parse_top(arguments: &[String]) -> usize {
    arguments
        .windows(2)
        .find(|pair| pair[0] == "--top")
        .and_then(|pair| pair[1].parse().ok())
        .unwrap_or(10)
}

fn print_help() {
    println!(
        "repo-radar\n\nUsage:\n  repo-radar [PATH] [--top N]\n\nSummarize files in a local repository.\n"
    );
}

fn print_summary(root: &Path, summary: &ScanReport) {
    println!("Repository: {}", root.display());
    println!("Files:      {}", summary.files);
    println!("Size:       {}", format_bytes(summary.bytes));

    println!("\nLanguages / extensions:");
    for (extension, count) in &summary.by_extension {
        println!("  {extension:<16} {count}");
    }

    println!("\nLargest files:");
    for file in &summary.largest_files {
        println!(
            "  {:>10}  {}",
            format_bytes(file.bytes),
            file.path.display()
        );
    }
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
}
