//! Integration tests for the warrant-core corpus classifier.
//!
//! This file must appear in `cargo test` output as `Running tests/corpus.rs`
//! (see `self_orphaned_mock_tests` memory — no orphaned mock subdir).

use std::path::PathBuf;

use warrant_core::{
    classify, ClaimKind, CloseRef, CloseSource, FakeSource, RawClose,
};

// ── AC2: classify is pure — zero CloseSource calls inside classify ──────────

/// CloseSource that panics if `close_notes` is ever called.
struct PanicSource;

impl CloseSource for PanicSource {
    fn close_notes(&self) -> Result<Vec<RawClose>, Box<dyn std::error::Error>> {
        panic!("classify must not call CloseSource::close_notes — it is pure");
    }
}

#[test]
fn classify_is_pure_no_source_calls() {
    // The caller calls close_notes; classify receives a plain &[RawClose].
    // This test verifies that classify itself never touches a CloseSource.
    let _panic_source = PanicSource; // exists but is never passed to classify
    let notes: Vec<RawClose> = vec![RawClose {
        reference: CloseRef::Prd(PathBuf::from("test/example.md")),
        text: "Status: Closed (2026-01-01)\n\nAll ACs verified.\n".to_string(),
    }];
    // classify takes &[RawClose] — no source argument — so PanicSource cannot be called
    let plan = classify(&notes);
    assert_eq!(plan.claims.len(), 1);
    assert_eq!(plan.claims[0].kind, ClaimKind::AcsMet);
}

// ── AC3: fixture classification — session-end-resilient + agentns-wrap ──────

#[test]
fn session_end_resilient_classified_as_mechanism_asserted() {
    let note = RawClose {
        reference: CloseRef::Prd(PathBuf::from(
            "PRDs-archive/PRD-ctrace-session-end-resilient.md",
        )),
        text: concat!(
            "Status: Closed (2026-06-02) — outcome achieved live by a different mechanism\n",
            "\n",
            "The ctrace-reap.timer now handles orphaned session logs. The reaper's\n",
            "`render_log` step summarizes any orphaned `*.ndjson` that session.bt left\n",
            "behind. The mechanism: ctrace-orphan-reap backfills summaries on a schedule.\n",
        )
        .to_string(),
    };

    let plan = classify(std::slice::from_ref(&note));
    assert_eq!(plan.claims.len(), 1);
    let claim = &plan.claims[0];
    assert_eq!(
        claim.kind,
        ClaimKind::MechanismAsserted,
        "session-end-resilient must be classified MechanismAsserted"
    );
    // mechanism_text captures the line containing the mechanism signal
    assert!(
        claim.mechanism_text.is_some(),
        "mechanism_text must be captured for MechanismAsserted"
    );
    let mech = claim.mechanism_text.as_deref().unwrap_or("");
    assert!(
        mech.to_lowercase().contains("mechanism")
            || mech.to_lowercase().contains("achieved"),
        "mechanism_text should reference the mechanism signal, got: {mech:?}"
    );
    assert_eq!(plan.tally.mechanism_asserted, 1);
}

#[test]
fn agentns_wrap_classified_as_superseded() {
    let note = RawClose {
        reference: CloseRef::Prd(PathBuf::from("PRDs-archive/PRD-agentns-wrap.md")),
        text: concat!(
            "Status: Closed (2026-06-06) — superseded by PRD-agentns-prctl.md\n",
            "\n",
            "The claude-agentns-wrap approach (CLONE_NEWAGENT=0x100 collision with CLONE_VM)\n",
            "is futile. The correct fix is the prctl path defined in the new PRD.\n",
        )
        .to_string(),
    };

    let plan = classify(std::slice::from_ref(&note));
    assert_eq!(plan.claims.len(), 1);
    let claim = &plan.claims[0];
    assert_eq!(
        claim.kind,
        ClaimKind::Superseded,
        "agentns-wrap must be classified Superseded"
    );
    assert_eq!(plan.tally.superseded, 1);
}

// ── AC4: AuditPlan is serde-serializable and has stable shape ───────────────

#[test]
fn audit_plan_serializes_to_json_with_expected_fields() {
    let source = FakeSource::with_illustrative_fixtures();
    let notes = source
        .close_notes()
        .expect("FakeSource::close_notes must not fail");
    let plan = classify(&notes);

    let json = serde_json::to_string_pretty(&plan).expect("AuditPlan must serialize to JSON");

    // Shape checks
    assert!(json.contains("\"claims\""), "JSON must have 'claims' key");
    assert!(json.contains("\"tally\""), "JSON must have 'tally' key");
    assert!(
        json.contains("\"mechanism_asserted\""),
        "tally must have mechanism_asserted"
    );
    assert!(json.contains("\"kind\""), "claims must have 'kind' field");
    assert!(json.contains("\"source\""), "claims must have 'source' field");

    // Round-trip
    let plan2: warrant_core::AuditPlan =
        serde_json::from_str(&json).expect("AuditPlan must round-trip from JSON");
    assert_eq!(plan.tally, plan2.tally);
    assert_eq!(plan.claims.len(), plan2.claims.len());
}

// ── ClaimKindTally helpers ───────────────────────────────────────────────────

#[test]
fn tally_total_sums_all_kinds() {
    use warrant_core::ClaimKindTally;
    let mut t = ClaimKindTally::default();
    t.increment(ClaimKind::AcsMet);
    t.increment(ClaimKind::MechanismAsserted);
    t.increment(ClaimKind::MechanismAsserted);
    t.increment(ClaimKind::Superseded);
    t.increment(ClaimKind::LiveDeferred);
    t.increment(ClaimKind::Unclassified);
    assert_eq!(t.total(), 6);
    assert_eq!(t.acs_met, 1);
    assert_eq!(t.mechanism_asserted, 2);
}

// ── CloseDate extraction ─────────────────────────────────────────────────────

#[test]
fn close_date_is_extracted() {
    let note = RawClose {
        reference: CloseRef::Prd(PathBuf::from("test/dated.md")),
        text: "Status: Closed (2026-03-15)\n\nAll ACs met.\n".to_string(),
    };
    let plan = classify(std::slice::from_ref(&note));
    assert_eq!(
        plan.claims[0].close_date.as_deref(),
        Some("2026-03-15"),
        "close_date must be extracted from the Status line"
    );
}

// ── LiveDeferred classification ──────────────────────────────────────────────

#[test]
fn live_deferred_classification() {
    let note = RawClose {
        reference: CloseRef::Prd(PathBuf::from("test/deferred.md")),
        text: "Status: Closed (2026-04-01)\n\ndeferred_acs: [3,4]\n\nPartial close.\n".to_string(),
    };
    let plan = classify(std::slice::from_ref(&note));
    assert_eq!(plan.claims[0].kind, ClaimKind::LiveDeferred);
    assert_eq!(plan.tally.live_deferred, 1);
}

// ── Unclassified when no status line ────────────────────────────────────────

#[test]
fn unclassified_when_no_status_line() {
    let note = RawClose {
        reference: CloseRef::Prd(PathBuf::from("test/noheader.md")),
        text: "Some random prose with no status line at all.\n".to_string(),
    };
    let plan = classify(std::slice::from_ref(&note));
    assert_eq!(plan.claims[0].kind, ClaimKind::Unclassified);
    assert_eq!(plan.tally.unclassified, 1);
}
