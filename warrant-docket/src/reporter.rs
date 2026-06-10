//! Core reporter logic for `warrant report`.
//!
//! Converts `Refuted` [`WarrantVerdict`]s into docket reopen requests, with
//! edge-triggering (no duplicate reports) and auto-close (Proven clears slug).

use std::path::Path;

use warrant_core::{
    CloseRef, DocketSink, ReportedState, WarrantStatus, WarrantVerdict,
};

/// The action taken for a single verdict entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportAction {
    /// The verdict will reopen (or would reopen, in dry-run) a docket entry.
    WouldReopen,
    /// The verdict was already reported in a previous run (edge-trigger no-op).
    AlreadyReported,
    /// The verdict is `Proven` and clears a previously-reported slug.
    AutoClosed,
    /// The verdict status does not trigger a docket action (`Unproven`/`Unwarranted`).
    NoAction,
}

/// A single entry in the report output.
#[derive(Debug, Clone)]
pub struct ReportEntry {
    /// The docket slug.
    pub slug: String,
    /// One-line summary.
    pub summary: String,
    /// Evidence string.
    pub evidence: String,
    /// What action was (or would be) taken.
    pub action: ReportAction,
}

/// The result of a `warrant report` pass.
#[derive(Debug, Clone)]
pub struct ReportResult {
    /// All entries, one per eligible verdict.
    pub entries: Vec<ReportEntry>,
    /// Number of actual reopen calls made (only non-zero with `--apply`).
    pub reopened: usize,
    /// Number of auto-closes performed.
    pub auto_closed: usize,
}

/// Derive the docket slug for a verdict source.
#[must_use]
pub fn slug_for(source: &CloseRef) -> String {
    match source {
        CloseRef::Prd(path) => {
            let basename = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            format!("warrant:{basename}")
        }
        CloseRef::Docket(s) => format!("warrant:{s}"),
    }
}

/// Derive a one-line summary for a `Refuted` verdict.
#[must_use]
pub fn summary_for(verdict: &WarrantVerdict) -> String {
    let source_str = match &verdict.source {
        CloseRef::Prd(path) => path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        CloseRef::Docket(s) => s.clone(),
    };
    format!(
        "close {source_str:?} claims mechanism delivered; assertion shows it is not ({})",
        verdict.kind
    )
}

/// Run the reporting pass over a list of verdicts.
///
/// - If `apply` is `true`, calls `sink.reopen(...)` for each new `Refuted` verdict
///   and updates the persisted state.
/// - If `apply` is `false`, only computes what *would* happen (print-only).
///
/// # Errors
///
/// Returns an error only if the state file cannot be read (not if the sink errors,
/// since we fail-open on sink errors).
pub fn report_verdicts(
    verdicts: &[WarrantVerdict],
    state_path: &Path,
    sink: &dyn DocketSink,
    apply: bool,
) -> Result<ReportResult, Box<dyn std::error::Error>> {
    let mut state = ReportedState::load(state_path)?;
    let mut entries = Vec::new();
    let mut reopened = 0usize;
    let mut auto_closed = 0usize;

    for verdict in verdicts {
        let slug = slug_for(&verdict.source);
        let status_str = format!("{:?}", verdict.status);

        match verdict.status {
            WarrantStatus::Refuted => {
                if state.already_reported(&slug, &status_str) {
                    entries.push(ReportEntry {
                        slug,
                        summary: summary_for(verdict),
                        evidence: verdict.evidence.clone(),
                        action: ReportAction::AlreadyReported,
                    });
                } else {
                    let summary = summary_for(verdict);
                    let action = if apply {
                        // Fail-open: if sink errors, print and continue
                        if let Err(e) = sink.reopen(&slug, &summary, &verdict.evidence) {
                            eprintln!("warrant report: docket sink error for {slug}: {e} (fail-open)");
                        } else {
                            reopened += 1;
                        }
                        state.mark_reported(&slug, &status_str);
                        ReportAction::WouldReopen
                    } else {
                        ReportAction::WouldReopen
                    };
                    entries.push(ReportEntry {
                        slug,
                        summary,
                        evidence: verdict.evidence.clone(),
                        action,
                    });
                }
            }
            WarrantStatus::Proven => {
                // Auto-close: if this slug was previously Refuted, clear it
                if state.reported.contains_key(&slug) {
                    if apply {
                        state.clear(&slug);
                        auto_closed += 1;
                    }
                    entries.push(ReportEntry {
                        slug,
                        summary: format!("verdict now Proven — auto-closing slug"),
                        evidence: verdict.evidence.clone(),
                        action: ReportAction::AutoClosed,
                    });
                } else {
                    entries.push(ReportEntry {
                        slug,
                        summary: String::new(),
                        evidence: verdict.evidence.clone(),
                        action: ReportAction::NoAction,
                    });
                }
            }
            _ => {
                // Unproven, Unwarranted, NotApplicable — no docket action
                entries.push(ReportEntry {
                    slug,
                    summary: String::new(),
                    evidence: verdict.evidence.clone(),
                    action: ReportAction::NoAction,
                });
            }
        }
    }

    if apply {
        // Persist the updated state, fail-open on error
        if let Err(e) = state.save(state_path) {
            eprintln!("warrant report: cannot save state to {}: {e} (fail-open)", state_path.display());
        }
    }

    Ok(ReportResult {
        entries,
        reopened,
        auto_closed,
    })
}
