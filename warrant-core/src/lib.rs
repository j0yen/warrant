//! warrant-core — close-claim domain model and pure classifier.
//!
//! This crate defines the core types for the warrant toolkit:
//! - [`CloseClaim`] — a parsed, typed close-note entry
//! - [`ClaimKind`] — the classification of a close claim
//! - [`CloseSource`] / [`RawClose`] — trait + input type for corpus readers
//! - [`FakeSource`] — in-memory fixture source for tests
//! - [`AuditPlan`] — the pure result of classifying a corpus
//! - [`Warrant`] / [`AssertionSpec`] / [`WarrantVerdict`] — assertion types
//! - [`classify`] — the pure, IO-free classifier function

pub mod classifier;
pub mod model;
pub mod source;

pub use classifier::classify;
pub use model::{
    AssertionSpec, AuditPlan, ClaimKind, ClaimKindTally, CloseClaim, CloseRef, Warrant,
    WarrantStatus, WarrantVerdict,
};
pub use source::{CloseSource, FakeSource, RawClose};
