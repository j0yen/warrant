//! warrant-audit — live source, registry loader, and assertion runner.
//!
//! This crate extends `warrant-core` with:
//! - [`FsDocketSource`] — reads `Status: Closed` PRDs from a filesystem archive
//! - [`RegistryLoader`] — loads `warrants.toml` mapping sources to assertions
//! - [`AssertionRunner`] — executes each [`AssertionSpec`] safely, yields [`WarrantVerdict`]
//! - [`AuditResult`] — the full result of running an audit pass

pub mod registry;
pub mod runner;
pub mod source;

pub use registry::{RegistryLoader, WarrantEntry, WarrantRegistry};
pub use runner::{run_audit, AuditResult, AuditTally};
pub use source::FsDocketSource;
