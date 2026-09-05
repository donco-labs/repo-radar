//! The `Html` type: the sole route from untrusted text into rendered markup.

use std::fmt;

/// A fragment of HTML that is safe to emit.
///
/// The only route from untrusted text to `Html` is [`Html::escape`], which
/// escapes on construction. Program-authored markup enters through
/// [`Html::from_static`], which requires a `&'static str` — a value derived
/// from repository content is never `'static`, so it cannot take that path.
///
/// This makes invariant I4 (`docs/specs/000-safety-invariants.md`) a property
/// the compiler checks rather than a rule every call site must remember.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Html(String);

impl Html {
    /// Escapes untrusted text. The only constructor that accepts a borrowed
    /// non-static string.
    pub fn escape(text: &str) -> Self {
        Self(
            text.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&#39;"),
        )
    }

    /// Markup the program itself authored. Never reachable from repository
    /// content, because such a value cannot be `'static`.
    pub fn from_static(markup: &'static str) -> Self {
        Self(markup.to_owned())
    }

    /// A number. Always safe; no escaping is possible or needed.
    pub fn number(value: u64) -> Self {
        Self(value.to_string())
    }

    /// Appends already-safe markup.
    pub fn push(&mut self, other: &Html) {
        self.0.push_str(&other.0);
    }

    /// Appends markup the program itself authored.
    pub fn push_static(&mut self, markup: &'static str) {
        self.0.push_str(markup);
    }

    /// Escapes `text` and appends it.
    pub fn push_escaped(&mut self, text: &str) {
        self.0.push_str(&Html::escape(text).0);
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Html {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escapes_repository_content() {
        assert_eq!(
            Html::escape("<script>\"&").as_str(),
            "&lt;script&gt;&quot;&amp;"
        );
    }

    #[test]
    fn escaping_already_escaped_text_does_not_special_case_it() {
        // `Html::escape` has no notion of "already escaped"; it applies the
        // same substitutions every time. Escaping text that already contains
        // entities re-encodes the leading `&` of each one. This test pins
        // that exact, unglamorous behavior rather than inventing an
        // idempotency rule the type does not implement.
        assert_eq!(
            Html::escape("&lt;script&gt;").as_str(),
            "&amp;lt;script&amp;gt;"
        );
    }
}
