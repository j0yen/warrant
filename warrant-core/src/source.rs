//! `CloseSource` trait and `FakeSource` in-memory fixture implementation.

use crate::model::CloseRef;

/// A raw, unclassified close note entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawClose {
    /// The source reference for this close note.
    pub reference: CloseRef,
    /// The raw text of the close note.
    pub text: String,
}

/// Trait for sources that can yield raw close notes.
///
/// The real filesystem/docket implementation lives in `warrant-audit`.
/// This crate ships only `FakeSource`.
pub trait CloseSource {
    /// Return all raw close notes from this source.
    ///
    /// # Errors
    ///
    /// Returns an error if the source cannot be read.
    fn close_notes(&self) -> Result<Vec<RawClose>, Box<dyn std::error::Error>>;
}

/// An in-memory fixture source for tests.
#[derive(Debug, Clone, Default)]
pub struct FakeSource {
    /// The pre-loaded raw close notes.
    pub notes: Vec<RawClose>,
}

impl FakeSource {
    /// Create a new `FakeSource` with the given notes.
    #[must_use]
    pub const fn new(notes: Vec<RawClose>) -> Self {
        Self { notes }
    }

    /// Create a `FakeSource` with the built-in illustrative fixtures.
    ///
    /// Includes verbatim excerpts of:
    /// - `PRD-ctrace-session-end-resilient.md` (`MechanismAsserted`)
    /// - `PRD-agentns-wrap.md` (Superseded / `MechanismAsserted`)
    #[must_use]
    pub fn with_illustrative_fixtures() -> Self {
        use std::path::PathBuf;

        let notes = vec![
            RawClose {
                reference: CloseRef::Prd(PathBuf::from(
                    "PRDs-archive/PRD-ctrace-session-end-resilient.md",
                )),
                text: concat!(
                    "Status: Closed (2026-06-02) — outcome achieved live by a different mechanism\n",
                    "\n",
                    "The ctrace-reap.timer now handles orphaned session logs. The reaper's\n",
                    "`render_log` step summarizes any orphaned `*.ndjson` that session.bt left\n",
                    "behind. The mechanism: ctrace-orphan-reap backfills summaries on a schedule.\n",
                ).to_string(),
            },
            RawClose {
                reference: CloseRef::Prd(PathBuf::from(
                    "PRDs-archive/PRD-agentns-wrap.md",
                )),
                text: concat!(
                    "Status: Closed (2026-06-06) — superseded by PRD-agentns-prctl.md\n",
                    "\n",
                    "The claude-agentns-wrap approach (CLONE_NEWAGENT=0x100 collision with CLONE_VM)\n",
                    "is futile. The correct fix is the prctl path defined in the new PRD.\n",
                    "This PRD is closed by vision-assay and the replacement PRD.\n",
                ).to_string(),
            },
            RawClose {
                reference: CloseRef::Prd(PathBuf::from(
                    "PRDs-archive/PRD-simple-acs-met.md",
                )),
                text: concat!(
                    "Status: Closed (2026-06-01)\n",
                    "\n",
                    "All acceptance criteria verified. The binary ships and tests pass.\n",
                ).to_string(),
            },
        ];

        Self { notes }
    }
}

impl CloseSource for FakeSource {
    fn close_notes(&self) -> Result<Vec<RawClose>, Box<dyn std::error::Error>> {
        Ok(self.notes.clone())
    }
}
