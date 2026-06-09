//! Core domain types for the warrant toolkit.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A reference to the source of a close claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum CloseRef {
    /// A path to a PRD file.
    Prd(PathBuf),
    /// A docket identifier string.
    Docket(String),
}

impl std::fmt::Display for CloseRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prd(p) => write!(f, "{}", p.display()),
            Self::Docket(s) => write!(f, "{s}"),
        }
    }
}

/// The classification of a close claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// All ACs were met; no special mechanism noted.
    AcsMet,
    /// "Achieved live by a different mechanism" — the canonical false-close risk.
    MechanismAsserted,
    /// Superseded by another PRD or vision.
    Superseded,
    /// Has `deferred_acs` in the close note.
    LiveDeferred,
    /// Could not be classified by any known pattern.
    Unclassified,
}

impl std::fmt::Display for ClaimKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AcsMet => write!(f, "AcsMet"),
            Self::MechanismAsserted => write!(f, "MechanismAsserted"),
            Self::Superseded => write!(f, "Superseded"),
            Self::LiveDeferred => write!(f, "LiveDeferred"),
            Self::Unclassified => write!(f, "Unclassified"),
        }
    }
}

/// A parsed, typed close-note entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseClaim {
    /// Where this close claim came from.
    pub source: CloseRef,
    /// The date the PRD was closed, if parseable.
    pub close_date: Option<String>,
    /// The classified kind of this close claim.
    pub kind: ClaimKind,
    /// The mechanism text, if `kind == MechanismAsserted`.
    pub mechanism_text: Option<String>,
    /// The outcome text, if present.
    pub outcome_text: Option<String>,
    /// The raw excerpt that was classified.
    pub raw_excerpt: String,
}

/// Per-kind tally of close claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClaimKindTally {
    /// Number of `AcsMet` claims.
    pub acs_met: usize,
    /// Number of `MechanismAsserted` claims.
    pub mechanism_asserted: usize,
    /// Number of `Superseded` claims.
    pub superseded: usize,
    /// Number of `LiveDeferred` claims.
    pub live_deferred: usize,
    /// Number of `Unclassified` claims.
    pub unclassified: usize,
}

impl ClaimKindTally {
    /// Total number of claims across all kinds.
    #[must_use]
    pub fn total(&self) -> usize {
        self.acs_met
            + self.mechanism_asserted
            + self.superseded
            + self.live_deferred
            + self.unclassified
    }

    /// Increment the counter for the given kind.
    pub fn increment(&mut self, kind: ClaimKind) {
        match kind {
            ClaimKind::AcsMet => self.acs_met += 1,
            ClaimKind::MechanismAsserted => self.mechanism_asserted += 1,
            ClaimKind::Superseded => self.superseded += 1,
            ClaimKind::LiveDeferred => self.live_deferred += 1,
            ClaimKind::Unclassified => self.unclassified += 1,
        }
    }
}

/// The pure result of classifying a corpus of close notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditPlan {
    /// All classified claims.
    pub claims: Vec<CloseClaim>,
    /// Per-kind tally.
    pub tally: ClaimKindTally,
}

/// The status of a warrant check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarrantStatus {
    /// The assertion was verified true.
    Proven,
    /// The assertion was verified false.
    Refuted,
    /// The assertion could not be evaluated.
    Unproven,
    /// The assertion was not applicable to this claim.
    Unwarranted,
    /// The assertion was not applicable (e.g., wrong claim kind).
    NotApplicable,
}

/// A verifiable assertion specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssertionSpec {
    /// Run a command and check its exit code.
    CommandExit {
        /// The shell command to run.
        cmd: String,
        /// The expected exit code.
        expect_code: i32,
    },
    /// Check that a file exists.
    FileExists {
        /// The path to check.
        path: PathBuf,
    },
    /// Check that a pattern is absent in a file.
    GrepAbsent {
        /// The file to search.
        path: PathBuf,
        /// The pattern to search for (must be absent).
        pattern: String,
    },
    /// Check that a counter in a file is non-zero.
    CounterNonzero {
        /// The file containing the counter.
        path: PathBuf,
        /// The key to look up.
        key: String,
    },
}

/// A warrant: a close claim paired with an assertion to verify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warrant {
    /// The close claim being warranted.
    pub source: CloseRef,
    /// The assertion to run against the claimed mechanism.
    pub assertion: AssertionSpec,
}

/// The verdict of running a warrant assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarrantVerdict {
    /// The close claim source.
    pub source: CloseRef,
    /// The kind of the close claim.
    pub kind: ClaimKind,
    /// The result of evaluating the assertion.
    pub status: WarrantStatus,
    /// Human-readable evidence string.
    pub evidence: String,
}
