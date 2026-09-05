//! Extension-to-language mapping used to group files into language families.
//!
//! The table is versioned static data (spec 003, "Unmapped extensions are
//! named, not guessed"): every entry is deliberate, and the version constant
//! travels with the JSON report so a consumer can tell whether a difference
//! in `by_language` came from the repository or from a table update.

/// Version of the extension-to-language table. Bump on any entry change.
pub const LANGUAGE_TABLE_VERSION: u32 = 1;

/// Files whose extension is not in the table.
pub const UNRECOGNIZED: &str = "[unrecognized]";

/// Files with no extension at all.
pub const NO_EXTENSION: &str = "[no extension]";

/// Extension-to-language table, sorted by extension for scannability.
///
/// Extensions are lowercase and dotless, matching the value the scanner
/// already computes from a file name.
const LANGUAGES: &[(&str, &str)] = &[
    ("bash", "Shell"),
    ("c", "C"),
    ("cc", "C++"),
    ("cjs", "JavaScript"),
    ("cpp", "C++"),
    ("csv", "CSV"),
    ("css", "CSS"),
    ("go", "Go"),
    ("h", "C"),
    ("hpp", "C++"),
    ("html", "HTML"),
    ("java", "Java"),
    ("js", "JavaScript"),
    ("json", "JSON"),
    ("jsx", "JavaScript"),
    ("kt", "Kotlin"),
    ("lock", "Lockfile"),
    ("md", "Markdown"),
    ("mjs", "JavaScript"),
    ("py", "Python"),
    ("rb", "Ruby"),
    ("rs", "Rust"),
    ("sh", "Shell"),
    ("sql", "SQL"),
    ("toml", "TOML"),
    ("ts", "TypeScript"),
    ("tsx", "TypeScript"),
    ("txt", "Plain text"),
    ("xml", "XML"),
    ("yaml", "YAML"),
    ("yml", "YAML"),
    ("zsh", "Shell"),
];

/// Resolves a lowercase, dotless extension to a language family.
///
/// Returns `None` when the extension is not in the table, including for the
/// `[no extension]` sentinel; the caller decides how to label that gap
/// (`NO_EXTENSION` versus `UNRECOGNIZED`), because the two are different and
/// are reported differently.
pub fn language_for_extension(extension: &str) -> Option<&'static str> {
    LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == extension)
        .map(|(_, language)| *language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_extensions() {
        assert_eq!(language_for_extension("rs"), Some("Rust"));
        assert_eq!(language_for_extension("yml"), Some("YAML"));
        assert_eq!(language_for_extension("yaml"), Some("YAML"));
        assert_eq!(language_for_extension("cc"), Some("C++"));
    }

    #[test]
    fn returns_none_for_unmapped_extensions() {
        assert_eq!(language_for_extension("xyz"), None);
        assert_eq!(language_for_extension(NO_EXTENSION), None);
    }
}
