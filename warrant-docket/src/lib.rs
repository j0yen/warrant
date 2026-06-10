//! warrant-docket — docket-sink reporter and edge-trigger state.
//!
//! This crate implements `warrant report`:
//! - Maps `Refuted` [`WarrantVerdict`]s to docket reopen requests
//! - Edge-triggers on state transitions (no duplicate reports per tick)
//! - Fail-open if the docket sink is absent
//! - Auto-closes slugs when a verdict flips back to `Proven`

pub mod reporter;

pub use reporter::{report_verdicts, ReportAction, ReportEntry, ReportResult};
