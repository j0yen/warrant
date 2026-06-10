//! Assertion runner — executes each [`AssertionSpec`] and yields [`WarrantVerdict`]s.
//!
//! Contracts:
//! - All assertions are **side-effect-free** (read/grep/exit-code only).
//! - Per-assertion failure is non-fatal: one bad warrant does not block others.
//! - A close present in the corpus but absent from the registry → `Unwarranted`.

use std::path::Path;

use warrant_core::{
    AssertionSpec, ClaimKind, CloseSource, WarrantStatus, WarrantVerdict,
};

use crate::registry::WarrantRegistry;

/// Tally of verdicts from one audit pass.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AuditTally {
    /// Number of `Proven` verdicts.
    pub proven: usize,
    /// Number of `Refuted` verdicts.
    pub refuted: usize,
    /// Number of `Unproven` verdicts.
    pub unproven: usize,
    /// Number of `Unwarranted` verdicts.
    pub unwarranted: usize,
}

impl AuditTally {
    /// Total number of verdicts.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.proven + self.refuted + self.unproven + self.unwarranted
    }

    fn increment(&mut self, status: WarrantStatus) {
        match status {
            WarrantStatus::Proven => self.proven += 1,
            WarrantStatus::Refuted => self.refuted += 1,
            WarrantStatus::Unproven => self.unproven += 1,
            WarrantStatus::Unwarranted => self.unwarranted += 1,
            WarrantStatus::NotApplicable => {}
        }
    }
}

/// The result of one full audit pass.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditResult {
    /// All verdicts, one per close claim in the corpus.
    pub verdicts: Vec<WarrantVerdict>,
    /// Summary tally.
    pub tally: AuditTally,
}

/// Run a single `AssertionSpec` and return a verdict.
///
/// This function is the only place that actually executes side-effect-free
/// assertions. It never writes, never mutates, never uses `--apply`.
fn run_one(spec: &AssertionSpec, source: &warrant_core::CloseRef) -> WarrantVerdict {
    let kind = ClaimKind::MechanismAsserted; // assertions are only attached to mechanism claims
    match spec {
        AssertionSpec::CommandExit { cmd, expect_code } => {
            run_command_exit(cmd, *expect_code, source, kind)
        }
        AssertionSpec::FileExists { path } => run_file_exists(path, source, kind),
        AssertionSpec::GrepAbsent { path, pattern } => {
            run_grep_absent(path, pattern, source, kind)
        }
        AssertionSpec::CounterNonzero { path, key } => {
            run_counter_nonzero(path, key, source, kind)
        }
    }
}

fn run_command_exit(
    cmd: &str,
    expect_code: i32,
    source: &warrant_core::CloseRef,
    kind: ClaimKind,
) -> WarrantVerdict {
    use std::process::Command;

    let result = Command::new("sh").arg("-c").arg(cmd).output();

    match result {
        Ok(output) => {
            let actual = output.status.code().unwrap_or(-1);
            if actual == expect_code {
                WarrantVerdict {
                    source: source.clone(),
                    kind,
                    status: WarrantStatus::Proven,
                    evidence: format!(
                        "command {cmd:?} exited {actual} (expected {expect_code}) — assertion holds"
                    ),
                }
            } else {
                WarrantVerdict {
                    source: source.clone(),
                    kind,
                    status: WarrantStatus::Refuted,
                    evidence: format!(
                        "command {cmd:?} exited {actual} but expected {expect_code} — assertion refuted"
                    ),
                }
            }
        }
        Err(e) => WarrantVerdict {
            source: source.clone(),
            kind,
            status: WarrantStatus::Unproven,
            evidence: format!("command {cmd:?} failed to execute: {e}"),
        },
    }
}

fn run_file_exists(
    path: &Path,
    source: &warrant_core::CloseRef,
    kind: ClaimKind,
) -> WarrantVerdict {
    if path.exists() {
        WarrantVerdict {
            source: source.clone(),
            kind,
            status: WarrantStatus::Proven,
            evidence: format!("file {} exists — assertion holds", path.display()),
        }
    } else {
        WarrantVerdict {
            source: source.clone(),
            kind,
            status: WarrantStatus::Refuted,
            evidence: format!("file {} does not exist — assertion refuted", path.display()),
        }
    }
}

fn run_grep_absent(
    path: &Path,
    pattern: &str,
    source: &warrant_core::CloseRef,
    kind: ClaimKind,
) -> WarrantVerdict {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            if content.contains(pattern) {
                // Pattern IS present — the "absent" assertion fails → Refuted
                WarrantVerdict {
                    source: source.clone(),
                    kind,
                    status: WarrantStatus::Refuted,
                    evidence: format!(
                        "pattern {pattern:?} found in {} — GrepAbsent assertion refuted (claimed mechanism absent, but old path persists)",
                        path.display()
                    ),
                }
            } else {
                // Pattern absent — assertion holds → Proven
                WarrantVerdict {
                    source: source.clone(),
                    kind,
                    status: WarrantStatus::Proven,
                    evidence: format!(
                        "pattern {pattern:?} absent from {} — GrepAbsent assertion holds",
                        path.display()
                    ),
                }
            }
        }
        Err(e) if path.exists() => WarrantVerdict {
            source: source.clone(),
            kind,
            status: WarrantStatus::Unproven,
            evidence: format!("cannot read {}: {e}", path.display()),
        },
        Err(_) => {
            // File absent on this build box — fail-open as Unproven (cloud-safe)
            WarrantVerdict {
                source: source.clone(),
                kind,
                status: WarrantStatus::Unproven,
                evidence: format!(
                    "file {} not found on this system (cloud-safe: assertion deferred)",
                    path.display()
                ),
            }
        }
    }
}

fn run_counter_nonzero(
    path: &Path,
    key: &str,
    source: &warrant_core::CloseRef,
    kind: ClaimKind,
) -> WarrantVerdict {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            // Look for a line containing "key: N" or "key = N"
            let found = content.lines().any(|line| {
                let lower = line.to_lowercase();
                if lower.contains(&key.to_lowercase()) {
                    // Extract numbers from the line
                    let num: Option<u64> = line
                        .split_whitespace()
                        .filter_map(|tok| tok.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok())
                        .find(|&n: &u64| n > 0);
                    num.is_some()
                } else {
                    false
                }
            });
            if found {
                WarrantVerdict {
                    source: source.clone(),
                    kind,
                    status: WarrantStatus::Proven,
                    evidence: format!(
                        "counter {key:?} is non-zero in {} — assertion holds",
                        path.display()
                    ),
                }
            } else {
                WarrantVerdict {
                    source: source.clone(),
                    kind,
                    status: WarrantStatus::Refuted,
                    evidence: format!(
                        "counter {key:?} is zero or absent in {} — assertion refuted",
                        path.display()
                    ),
                }
            }
        }
        Err(_) => WarrantVerdict {
            source: source.clone(),
            kind,
            status: WarrantStatus::Unproven,
            evidence: format!(
                "file {} not found (cloud-safe: assertion deferred)",
                path.display()
            ),
        },
    }
}

/// Status filter for the audit output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    /// Show all verdicts.
    All,
    /// Show only `Refuted` verdicts.
    Refuted,
    /// Show only `Unwarranted` verdicts.
    Unwarranted,
}

/// Run a full audit pass over a corpus source and registry.
///
/// # Errors
///
/// Returns an error if the close source cannot be read.
pub fn run_audit(
    source: &dyn CloseSource,
    registry: &WarrantRegistry,
    filter: StatusFilter,
) -> Result<AuditResult, Box<dyn std::error::Error>> {
    let raw_notes = source.close_notes()?;
    let mut verdicts = Vec::new();
    let mut tally = AuditTally::default();

    for raw in &raw_notes {
        // Derive the basename from the source reference
        let basename = match &raw.reference {
            warrant_core::CloseRef::Prd(path) => path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            warrant_core::CloseRef::Docket(s) => s.clone(),
        };

        let verdict = if let Some(entry) = registry.find(&basename) {
            // Classify this claim — we need to know its kind for context
            let notes_slice = std::slice::from_ref(raw);
            let plan = warrant_core::classify(notes_slice);
            let claim_kind = plan
                .claims
                .first()
                .map_or(ClaimKind::Unclassified, |c| c.kind);

            let mut v = run_one(&entry.assertion, &raw.reference);
            v.kind = claim_kind;
            v
        } else {
            // No registry entry → Unwarranted
            let notes_slice = std::slice::from_ref(raw);
            let plan = warrant_core::classify(notes_slice);
            let claim_kind = plan
                .claims
                .first()
                .map_or(ClaimKind::Unclassified, |c| c.kind);

            WarrantVerdict {
                source: raw.reference.clone(),
                kind: claim_kind,
                status: WarrantStatus::Unwarranted,
                evidence: format!("no warrant registered for {basename}"),
            }
        };

        tally.increment(verdict.status);
        verdicts.push(verdict);
    }

    // Apply filter
    if filter != StatusFilter::All {
        verdicts.retain(|v| match filter {
            StatusFilter::Refuted => v.status == WarrantStatus::Refuted,
            StatusFilter::Unwarranted => v.status == WarrantStatus::Unwarranted,
            StatusFilter::All => true,
        });
    }

    Ok(AuditResult { verdicts, tally })
}
