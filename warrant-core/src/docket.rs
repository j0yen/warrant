//! `DocketSink` trait and implementations for reporting verdicts to the docket ledger.
//!
//! Design principles:
//! - **Fail-open**: if the docket sink is absent or errors, `warrant report` prints
//!   the would-be entries and exits 0. Never block on a missing sink.
//! - **Edge-triggered**: a small persisted state file remembers which `(slug, status)`
//!   pairs were already reported, so re-running does not re-open an already-tracked finding.
//! - **Auto-close**: a verdict that flips back to `Proven` clears its slug from state.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A sink that accepts docket reopen requests.
///
/// `FakeDocketSink` (for tests) records calls in-memory.
/// A real impl shells out to the `docket` binary or links its producer API.
pub trait DocketSink {
    /// Reopen (or create) a docket entry under `slug`.
    ///
    /// # Errors
    ///
    /// Returns an error if the sink cannot process the request. Callers must
    /// handle errors with fail-open semantics (print + exit 0).
    fn reopen(&self, slug: &str, summary: &str, evidence: &str) -> Result<(), Box<dyn std::error::Error>>;
}

/// A `DocketSink` that records calls for use in tests.
#[derive(Debug, Default)]
pub struct FakeDocketSink {
    /// All recorded `reopen` calls: `(slug, summary, evidence)`.
    pub calls: std::sync::Mutex<Vec<(String, String, String)>>,
}

impl FakeDocketSink {
    /// Create a new empty `FakeDocketSink`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return all recorded calls as a snapshot.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned (only possible in buggy tests).
    #[must_use]
    pub fn recorded_calls(&self) -> Vec<(String, String, String)> {
        #[allow(clippy::expect_used)]
        self.calls.lock().expect("FakeDocketSink mutex poisoned").clone()
    }
}

impl DocketSink for FakeDocketSink {
    fn reopen(
        &self,
        slug: &str,
        summary: &str,
        evidence: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[allow(clippy::expect_used)]
        self.calls
            .lock()
            .expect("FakeDocketSink mutex poisoned")
            .push((slug.to_string(), summary.to_string(), evidence.to_string()));
        Ok(())
    }
}

/// A `DocketSink` that prints what it would report (dry-run / fail-open).
#[derive(Debug, Default)]
pub struct PrintDocketSink;

impl DocketSink for PrintDocketSink {
    fn reopen(
        &self,
        slug: &str,
        summary: &str,
        evidence: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[allow(clippy::print_stdout)]
        {
            println!("[docket reopen] slug={slug}");
            println!("  summary:  {summary}");
            println!("  evidence: {evidence}");
        }
        Ok(())
    }
}

// ── Edge-trigger state ────────────────────────────────────────────────────────

/// Persisted state for edge-triggering: which `(slug, status)` pairs have
/// already been reported to the docket.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportedState {
    /// Map from `slug` to the last-reported status string.
    #[serde(default)]
    pub reported: HashMap<String, String>,
}

impl ReportedState {
    /// Load from a JSON file, or return an empty state if the file is absent.
    ///
    /// # Errors
    ///
    /// Returns an error only if the file exists but cannot be parsed.
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        let state = serde_json::from_str(&text)?;
        Ok(state)
    }

    /// Persist to a JSON file, creating parent directories as needed.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Return `true` if this `(slug, status)` pair has already been reported.
    #[must_use]
    pub fn already_reported(&self, slug: &str, status: &str) -> bool {
        self.reported.get(slug).is_some_and(|s| s == status)
    }

    /// Record that `(slug, status)` has been reported.
    pub fn mark_reported(&mut self, slug: &str, status: &str) {
        self.reported.insert(slug.to_string(), status.to_string());
    }

    /// Clear a slug from the state (auto-close: verdict flipped back to Proven).
    pub fn clear(&mut self, slug: &str) {
        self.reported.remove(slug);
    }
}

/// Default path for the persisted reported state.
#[must_use]
pub fn default_state_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(format!("{home}/.local/state/warrant/reported.json"))
}
