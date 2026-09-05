//! Assembles the standalone HTML dashboard.
//!
//! The document is a single self-contained file: the stylesheet is inlined
//! and no request leaves the machine. Every value drawn from repository
//! content passes through [`Html::escape`] before it reaches the page, which
//! is what keeps a hostile file name from becoming markup (invariant I4).

mod markup;

pub use markup::Html;

use std::fmt;
use std::path::Path;

use crate::{ScanReport, display_path};

use super::format_bytes;

/// The stylesheet, inlined at compile time. `include_str!` resolves relative
/// to this file, so the path is just the sibling file name.
const STYLE: &str = include_str!("style.css");

pub fn write_html(out: &mut impl fmt::Write, root: &Path, report: &ScanReport) -> fmt::Result {
    write!(out, "{}", document(root, report))
}

fn document(root: &Path, report: &ScanReport) -> Html {
    let repository = Html::escape(&display_path(root));
    let lines_stat = if report.lines.evaluated {
        Html::number(report.lines.lines)
    } else {
        Html::from_static("not evaluated")
    };

    let mut html = Html::default();
    html.push_static("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>Repo Radar | ");
    html.push(&repository);
    html.push_static("</title>\n<style>\n");
    html.push_static(STYLE);
    html.push_static("</style>\n</head>\n<body>\n<main>\n<header><div><div class=\"eyebrow\">Repository observatory</div><h1>Repo Radar</h1><p class=\"path\">");
    html.push(&repository);
    html.push_static("</p></div><div class=\"badge\">READ-ONLY SCAN</div></header>\n<section class=\"stats\" aria-label=\"Repository totals\">\n<div class=\"stat\"><strong>");
    html.push(&Html::number(report.files as u64));
    html.push_static("</strong><span>files inventoried</span></div>\n<div class=\"stat\"><strong>");
    html.push_escaped(&format_bytes(report.bytes));
    html.push_static("</strong><span>total size</span></div>\n<div class=\"stat\"><strong>");
    html.push(&Html::number(report.by_extension.len() as u64));
    html.push_static(
        "</strong><span>extensions detected</span></div>\n<div class=\"stat\"><strong>",
    );
    html.push(&lines_stat);
    html.push_static(
        "</strong><span>lines counted</span></div>\n</section>\n<section class=\"grid\">\n<article class=\"panel\"><h2>Composition</h2>",
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
        html.push_static("<div class=\"bar-row\"><span class=\"bar-label\">");
        html.push_escaped(&language.language);
        html.push_static("</span><span class=\"bar-track\"><span class=\"bar\" style=\"width:");
        html.push_escaped(&format!("{percentage:.1}"));
        html.push_static("%\"></span></span><strong>");
        html.push_escaped(&format_bytes(language.bytes));
        html.push_static("</strong></div>");
    }
    if report.by_language.is_empty() {
        html.push_static("<p class=\"empty\">No files found.</p>");
    }

    html.push_static("</article><article class=\"panel\"><h2>Largest files</h2>");
    if report.largest_files.is_empty() {
        html.push_static("<p class=\"empty\">No files found.</p>");
    } else {
        html.push_static("<table><thead><tr><th>Path</th><th>Size</th></tr></thead><tbody>");
        for file in &report.largest_files {
            html.push_static("<tr><td><code>");
            html.push_escaped(&display_path(&file.path));
            html.push_static("</code></td><td>");
            html.push_escaped(&format_bytes(file.bytes));
            html.push_static("</td></tr>");
        }
        html.push_static("</tbody></table>");
    }
    html.push_static(
        "</article><article class=\"panel panel-wide\"><h2>Largest directories (aggregate, including subdirectories)</h2>",
    );
    if report.largest_directories.is_empty() {
        html.push_static("<p class=\"empty\">No files found.</p>");
    } else {
        html.push_static("<table><thead><tr><th>Path</th><th>Size</th></tr></thead><tbody>");
        for directory in &report.largest_directories {
            html.push_static("<tr><td><code>");
            html.push_escaped(&display_path(&directory.path));
            html.push_static("</code></td><td>");
            html.push_escaped(&format_bytes(directory.bytes));
            html.push_static("</td></tr>");
        }
        html.push_static("</tbody></table>");
    }
    html.push_static("</article><article class=\"panel panel-wide\"><h2>Scan notes</h2>");
    if report.warnings.is_empty() {
        html.push_static("<p class=\"empty\">No warnings. The scan completed cleanly.</p>");
    } else {
        for warning in &report.warnings {
            html.push_static("<p class=\"warning\"><code>");
            html.push_escaped(&display_path(&warning.path));
            html.push_static("</code>: ");
            html.push_escaped(&warning.message);
            html.push_static("</p>");
        }
    }
    html.push_static("</article></section><footer>Generated by Repo Radar ");
    html.push_static(env!("CARGO_PKG_VERSION"));
    html.push_static(
        ". This report contains no external assets or requests.</footer></main></body></html>",
    );

    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileEntry;
    use std::path::PathBuf;

    #[test]
    fn html_renders_repository_content_as_inert_text() {
        let report = ScanReport {
            largest_files: vec![FileEntry {
                path: PathBuf::from("<script>evil</script>.rs"),
                bytes: 10,
            }],
            ..ScanReport::default()
        };

        let mut output = String::new();
        write_html(&mut output, Path::new("."), &report).expect("writing to a String cannot fail");

        assert!(!output.contains("<script>evil</script>"));
        assert!(output.contains("&lt;script&gt;evil&lt;/script&gt;.rs"));
    }
}
