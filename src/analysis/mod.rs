//! Analyses that extend the core scan model.
//!
//! Each analysis here is a [`crate::Analysis`]: it either ran and produced a
//! result, or it names why it did not, rather than standing in a plausible
//! zero (invariant I10, `docs/specs/000-safety-invariants.md`).

pub mod cargo;
pub mod git;
