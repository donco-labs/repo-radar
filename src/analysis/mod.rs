//! Analyses that extend the core scan model.
//!
//! Each analysis here is a [`crate::Analysis`]: it either ran and produced a
//! result, or it names why it did not, rather than standing in a plausible
//! zero (invariant I10, `docs/specs/000-safety-invariants.md`).
//!
//! [`Confidence`] and [`read_bounded`] live here, not in any one analysis
//! module, because both are cross-analysis seams: spec 014's 6a parcel
//! (`profile.rs`) establishes them on a single finding, and its 6c parcel
//! reuses them across an entire stack detector table rather than inventing a
//! second shape.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::NotEvaluated;

pub mod cargo;
pub mod git;
pub mod profile;

/// How much weight a finding's evidence carries.
///
/// Spec 014 criterion 2: `Certain` is reserved for a value a manifest or a
/// lockfile *declares*. Anything read out of prose, inferred from a naming
/// convention, or produced by a heuristic is `Inferred`, however obvious it
/// looks. The distinction is the difference between reporting evidence and
/// reporting a guess, which is the line spec 014 exists to hold.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Declared by a manifest or lockfile entry.
    Certain,
    /// Read from prose, a naming convention, or a heuristic.
    #[default]
    Inferred,
}

impl Confidence {
    /// The stable lowercase token, matching the JSON contract's spelling.
    pub fn label(self) -> &'static str {
        match self {
            Self::Certain => "certain",
            Self::Inferred => "inferred",
        }
    }
}

/// Reads `path` as UTF-8 text, refusing anything over `max_bytes` via
/// [`fs::metadata`] *before* the content is read (invariant I9). `label` is
/// the file's own name, used only to build a tool-authored detail string —
/// never anything read from the file.
pub(crate) fn read_bounded(
    path: &Path,
    label: &str,
    max_bytes: u64,
) -> Result<String, NotEvaluated> {
    let missing = || NotEvaluated::InputUnavailable(format!("no {label} at the repository root"));

    let metadata = fs::metadata(path).map_err(|_| missing())?;
    if !metadata.is_file() {
        return Err(missing());
    }
    if metadata.len() > max_bytes {
        return Err(NotEvaluated::Failed(format!(
            "{label} exceeds the size limit"
        )));
    }

    let bytes =
        fs::read(path).map_err(|_| NotEvaluated::Failed(format!("{label} could not be read")))?;
    String::from_utf8(bytes)
        .map_err(|_| NotEvaluated::Failed(format!("{label} is not valid UTF-8")))
}
