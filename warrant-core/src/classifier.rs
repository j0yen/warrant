//! Pure close-note classifier.
//!
//! `classify` is deliberately IO-free: it takes pre-fetched `&[RawClose]` slices
//! and performs only pattern matching. No file access, no clock, no env reads.

use crate::model::{AuditPlan, ClaimKind, ClaimKindTally, CloseClaim};
use crate::source::RawClose;

/// Patterns that signal a `MechanismAsserted` close.
const MECHANISM_PATTERNS: &[&str] = &[
    "achieved live by a different mechanism",
    "outcome achieved",
    "delivered by ",
    "mechanism:",
];

/// Patterns that signal a `Superseded` close.
const SUPERSEDED_PATTERNS: &[&str] = &[
    "superseded by",
    "closed … by prd-",
    "closed by prd-",
    "closed by vision-",
    "by prd-",
    "by vision-",
    "replaced by",
];

/// Patterns that signal a `LiveDeferred` close.
const LIVE_DEFERRED_PATTERNS: &[&str] = &["deferred_acs:"];

/// Extract a short excerpt around a keyword match (at most one line).
fn extract_mechanism_text(text: &str) -> Option<String> {
    for line in text.lines() {
        let lower = line.to_lowercase();
        for pat in MECHANISM_PATTERNS {
            if lower.contains(pat) {
                return Some(line.trim().to_owned());
            }
        }
    }
    None
}

/// Extract the close date from a `Status: Closed (YYYY-MM-DD)` line.
fn extract_close_date(text: &str) -> Option<String> {
    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("status: closed") {
            // Try to find a parenthesized date like (2026-06-02)
            if let Some(start) = line.find('(') {
                if let Some(end) = line[start..].find(')') {
                    let candidate = &line[start + 1..start + end];
                    // Rough ISO-date check: YYYY-MM-DD
                    if candidate.len() == 10
                        && candidate.as_bytes().get(4).copied() == Some(b'-')
                        && candidate.as_bytes().get(7).copied() == Some(b'-')
                    {
                        return Some(candidate.to_owned());
                    }
                }
            }
        }
    }
    None
}

/// Classify a single raw close note.
///
/// Precedence (highest first):
/// 1. `MechanismAsserted` — "achieved live by a different mechanism"
/// 2. `Superseded` — "superseded" / "closed by PRD-" / "by vision-"
/// 3. `LiveDeferred` — `deferred_acs:` present
/// 4. `AcsMet` — status closed, no special signal
/// 5. `Unclassified` — no status line found
#[must_use]
pub fn classify_one(raw: &RawClose) -> CloseClaim {
    let text_lower = raw.text.to_lowercase();
    let close_date = extract_close_date(&raw.text);

    let is_closed = text_lower.contains("status: closed");

    // 1. MechanismAsserted
    let has_mechanism = MECHANISM_PATTERNS
        .iter()
        .any(|p| text_lower.contains(p));

    // 2. Superseded
    let has_superseded = SUPERSEDED_PATTERNS
        .iter()
        .any(|p| text_lower.contains(p));

    // 3. LiveDeferred
    let has_deferred = LIVE_DEFERRED_PATTERNS
        .iter()
        .any(|p| text_lower.contains(p));

    let kind = if has_mechanism {
        ClaimKind::MechanismAsserted
    } else if has_superseded {
        ClaimKind::Superseded
    } else if has_deferred {
        ClaimKind::LiveDeferred
    } else if is_closed {
        ClaimKind::AcsMet
    } else {
        ClaimKind::Unclassified
    };

    let mechanism_text = if kind == ClaimKind::MechanismAsserted {
        extract_mechanism_text(&raw.text)
    } else {
        None
    };

    // outcome_text: first non-empty line after the status line
    let outcome_text = {
        let mut found_status = false;
        let mut result = None;
        for line in raw.text.lines() {
            if found_status && !line.trim().is_empty() {
                result = Some(line.trim().to_owned());
                break;
            }
            if line.to_lowercase().starts_with("status:") {
                found_status = true;
            }
        }
        result
    };

    CloseClaim {
        source: raw.reference.clone(),
        close_date,
        kind,
        mechanism_text,
        outcome_text,
        raw_excerpt: raw.text.clone(),
    }
}

/// Classify a slice of raw close notes into an [`AuditPlan`].
///
/// This function is **pure**: it performs no IO, reads no files, consults no
/// clock or environment variables. The caller is responsible for obtaining the
/// `&[RawClose]` slice from a [`crate::CloseSource`].
#[must_use]
pub fn classify(notes: &[RawClose]) -> AuditPlan {
    let mut tally = ClaimKindTally::default();
    let claims: Vec<CloseClaim> = notes
        .iter()
        .map(|raw| {
            let claim = classify_one(raw);
            tally.increment(claim.kind);
            claim
        })
        .collect();

    AuditPlan { claims, tally }
}
