//! Turns a [`crate::ScanReport`] into text, JSON, or HTML.
//!
//! Every renderer writes into a caller-supplied [`std::fmt::Write`] sink and
//! returns the sink's error, so the same code serves `stdout` today and a
//! socket later (spec 006). No renderer reads the filesystem, runs Git, or
//! opens a connection of its own — it only formats the model it is given.

pub mod html;
pub mod json;
pub mod text;

/// Formats a byte count as B, KiB, MiB, or GiB.
pub fn format_bytes(bytes: u64) -> String {
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
    use crate::ScanReport;
    use std::path::Path;

    #[test]
    fn formats_bytes_for_humans() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KiB");
    }

    #[test]
    fn renderers_are_reusable_across_sinks() {
        let report = ScanReport::default();
        let root = Path::new(".");

        let mut text_a = String::new();
        let mut text_b = String::new();
        text::write_summary(&mut text_a, root, &report).expect("writing to a String cannot fail");
        text::write_summary(&mut text_b, root, &report).expect("writing to a String cannot fail");
        assert_eq!(text_a, text_b);

        let mut json_a = String::new();
        let mut json_b = String::new();
        json::write_json(&mut json_a, root, &report).expect("writing to a String cannot fail");
        json::write_json(&mut json_b, root, &report).expect("writing to a String cannot fail");
        assert_eq!(json_a, json_b);

        let mut html_a = String::new();
        let mut html_b = String::new();
        html::write_html(&mut html_a, root, &report).expect("writing to a String cannot fail");
        html::write_html(&mut html_b, root, &report).expect("writing to a String cannot fail");
        assert_eq!(html_a, html_b);
    }
}
