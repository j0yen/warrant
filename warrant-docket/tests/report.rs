//! Integration tests for warrant-docket reporter.
//!
//! This file must appear in `cargo test` output as `Running tests/report.rs`
//! (see `self_orphaned_mock_tests` memory — no orphaned mock subdir).

// Tests legitimately use expect/unwrap/panic — allow them here.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use warrant_core::{ClaimKind, CloseRef, FakeDocketSink, WarrantStatus, WarrantVerdict};
use warrant_docket::{report_verdicts, ReportAction};

// ── helpers ───────────────────────────────────────────────────────────────────

fn refuted_verdict(source_basename: &str) -> WarrantVerdict {
    WarrantVerdict {
        source: CloseRef::Prd(PathBuf::from(format!(
            "PRDs-archive/{source_basename}"
        ))),
        kind: ClaimKind::MechanismAsserted,
        status: WarrantStatus::Refuted,
        evidence: format!("test evidence for {source_basename}"),
    }
}

fn proven_verdict(source_basename: &str) -> WarrantVerdict {
    WarrantVerdict {
        source: CloseRef::Prd(PathBuf::from(format!(
            "PRDs-archive/{source_basename}"
        ))),
        kind: ClaimKind::MechanismAsserted,
        status: WarrantStatus::Proven,
        evidence: format!("test evidence proven for {source_basename}"),
    }
}

fn temp_state_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("reported.json");
    (dir, path)
}

// ── AC2: Refuted → reopen once; Proven → no reopen ───────────────────────────

#[test]
fn refuted_verdict_reopens_once_with_apply() {
    let (_dir, state_path) = temp_state_path();
    let sink = FakeDocketSink::new();
    let verdicts = vec![refuted_verdict("PRD-ctrace-session-end-resilient.md")];

    let result =
        report_verdicts(&verdicts, &state_path, &sink, /*apply=*/ true).expect("report");

    let calls = sink.recorded_calls();
    assert_eq!(calls.len(), 1, "Refuted verdict must produce exactly one reopen call");
    let (slug, _summary, evidence) = calls.first().expect("one call");
    assert!(
        slug.contains("ctrace-session-end-resilient"),
        "slug must reference the source basename, got: {slug}"
    );
    assert!(
        evidence.contains("ctrace-session-end-resilient"),
        "evidence must be passed through, got: {evidence}"
    );
    assert_eq!(result.reopened, 1);
}

#[test]
fn proven_verdict_produces_no_reopen_call() {
    let (_dir, state_path) = temp_state_path();
    let sink = FakeDocketSink::new();
    let verdicts = vec![proven_verdict("PRD-some-thing.md")];

    let result =
        report_verdicts(&verdicts, &state_path, &sink, /*apply=*/ true).expect("report");

    let calls = sink.recorded_calls();
    assert!(
        calls.is_empty(),
        "Proven verdict must produce zero reopen calls, got: {calls:?}"
    );
    assert_eq!(result.reopened, 0);
}

// ── AC3: edge-trigger — second run is no-op ───────────────────────────────────

#[test]
fn second_apply_over_same_refuted_is_noop() {
    let (_dir, state_path) = temp_state_path();
    let sink = FakeDocketSink::new();
    let verdicts = vec![refuted_verdict("PRD-ctrace-session-end-resilient.md")];

    // First run — should reopen once
    let r1 = report_verdicts(&verdicts, &state_path, &sink, true).expect("first report");
    assert_eq!(r1.reopened, 1);

    // Second run — state persisted, must be a no-op
    let r2 = report_verdicts(&verdicts, &state_path, &sink, true).expect("second report");
    assert_eq!(r2.reopened, 0, "second run must be a no-op (edge-triggered)");

    // Total reopen calls: still 1
    let calls = sink.recorded_calls();
    assert_eq!(calls.len(), 1, "reopen must be called exactly once across two runs");

    // The second run entry must be AlreadyReported
    let entry = r2.entries.first().expect("entry");
    assert_eq!(
        entry.action,
        ReportAction::AlreadyReported,
        "second run must mark entry AlreadyReported"
    );
}

// ── AC4: auto-close — Proven clears slug from state ──────────────────────────

#[test]
fn proven_after_refuted_clears_slug() {
    let (_dir, state_path) = temp_state_path();
    let sink = FakeDocketSink::new();

    // First: Refuted → mark in state
    let verdicts_refuted = vec![refuted_verdict("PRD-ctrace-session-end-resilient.md")];
    let r1 = report_verdicts(&verdicts_refuted, &state_path, &sink, true).expect("first");
    assert_eq!(r1.reopened, 1);

    // Now Proven → auto-close clears the slug
    let sink2 = FakeDocketSink::new();
    let verdicts_proven = vec![proven_verdict("PRD-ctrace-session-end-resilient.md")];
    let r2 = report_verdicts(&verdicts_proven, &state_path, &sink2, true).expect("second");

    assert_eq!(r2.auto_closed, 1, "auto-close must fire when Proven after Refuted");
    let entry = r2.entries.first().expect("entry");
    assert_eq!(
        entry.action,
        ReportAction::AutoClosed,
        "entry must be marked AutoClosed"
    );

    // State file must no longer contain the slug
    let state = warrant_core::ReportedState::load(&state_path).expect("load state");
    let slug = "warrant:PRD-ctrace-session-end-resilient";
    assert!(
        !state.reported.contains_key(slug),
        "slug must be cleared from state after auto-close"
    );
}

// ── AC5: fail-open — no docket sink available → print + exit 0 ───────────────

/// A sink that always errors.
struct ErrorSink;

impl warrant_core::DocketSink for ErrorSink {
    fn reopen(
        &self,
        _slug: &str,
        _summary: &str,
        _evidence: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("simulated docket sink unavailable".into())
    }
}

#[test]
fn fail_open_with_erroring_sink_exits_ok() {
    let (_dir, state_path) = temp_state_path();
    let sink = ErrorSink;
    let verdicts = vec![refuted_verdict("PRD-ctrace-session-end-resilient.md")];

    // Must not return Err — fail-open means we print + exit 0
    let result = report_verdicts(&verdicts, &state_path, &sink, /*apply=*/ true);
    assert!(
        result.is_ok(),
        "fail-open: erroring sink must not propagate error, got: {result:?}"
    );
}

// ── AC6: print-only (no --apply) → zero reopen calls ─────────────────────────

#[test]
fn print_only_without_apply_produces_zero_reopen_calls() {
    let (_dir, state_path) = temp_state_path();
    let sink = FakeDocketSink::new();
    let verdicts = vec![
        refuted_verdict("PRD-ctrace-session-end-resilient.md"),
        refuted_verdict("PRD-another-thing.md"),
    ];

    let result =
        report_verdicts(&verdicts, &state_path, &sink, /*apply=*/ false).expect("report");

    let calls = sink.recorded_calls();
    assert!(
        calls.is_empty(),
        "print-only mode must produce zero reopen calls on FakeDocketSink, got: {calls:?}"
    );
    assert_eq!(result.reopened, 0);

    // Entries must still be listed as WouldReopen
    for entry in &result.entries {
        assert_eq!(
            entry.action,
            ReportAction::WouldReopen,
            "print-only entries must be WouldReopen"
        );
    }
}
