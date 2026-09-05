use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, PartialEq)]
struct RepositorySummary {
    files: usize,
    bytes: u64,
    by_extension: BTreeMap<String, usize>,
    largest_files: Vec<FileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileEntry {
    path: PathBuf,
    bytes: u64,
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

    let root = arguments
        .first()
        .filter(|argument| !argument.starts_with('-'))
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let top = parse_top(&arguments);

    match summarize(&root, top) {
        Ok(summary) => print_summary(&root, &summary),
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

fn summarize(root: &Path, top: usize) -> io::Result<RepositorySummary> {
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is not a directory", root.display()),
        ));
    }

    let mut summary = RepositorySummary::default();
    let mut largest_files = Vec::new();
    scan_directory(root, root, &mut summary, &mut largest_files)?;

    largest_files.sort_by_key(|file| Reverse(file.bytes));
    summary.largest_files = largest_files.into_iter().take(top).collect();
    Ok(summary)
}

fn scan_directory(
    root: &Path,
    directory: &Path,
    summary: &mut RepositorySummary,
    largest_files: &mut Vec<FileEntry>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_symlink() || should_skip(&path) {
            continue;
        }

        if file_type.is_dir() {
            scan_directory(root, &path, summary, largest_files)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let bytes = entry.metadata()?.len();
        summary.files += 1;
        summary.bytes += bytes;

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("[no extension]")
            .to_ascii_lowercase();
        *summary.by_extension.entry(extension).or_default() += 1;

        largest_files.push(FileEntry {
            path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
            bytes,
        });
    }

    Ok(())
}

fn should_skip(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target" | "node_modules"))
}

fn print_summary(root: &Path, summary: &RepositorySummary) {
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
    fn skips_build_and_dependency_directories() {
        assert!(should_skip(Path::new("target")));
        assert!(should_skip(Path::new("node_modules")));
        assert!(!should_skip(Path::new("src")));
    }

    #[test]
    fn formats_bytes_for_humans() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KiB");
    }
}
