//! `warrants.toml` registry loader.
//!
//! Format:
//! ```toml
//! [[warrant]]
//! source = "PRD-ctrace-session-end-resilient.md"
//!
//! [warrant.assertion]
//! type = "grep_absent"
//! path = "/home/user/.claude/scripts/ctrace-session-end.sh"
//! pattern = "scribe"
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warrant_core::AssertionSpec;

/// Mutating verbs forbidden in `CommandExit` commands.
const MUTATING_VERBS: &[&str] = &[
    "--apply",
    " rm ",
    "\nrm ",
    "^rm ",
    " rm\n",
    " rm$",
    ">",
    "write",
    "mkdir",
    "mv ",
    "cp ",
    "chmod",
    "chown",
    "dd ",
    "install ",
    "sudo",
    "tee ",
    "truncate",
    "shred",
    "mktemp",
    "curl ",
    "wget ",
];

/// A single warrant entry from `warrants.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarrantEntry {
    /// The source filename (basename of the PRD or docket slug).
    pub source: String,
    /// The assertion to run.
    pub assertion: AssertionSpec,
}

/// The full parsed registry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WarrantRegistry {
    /// All warrant entries.
    #[serde(default, rename = "warrant")]
    pub entries: Vec<WarrantEntry>,
}

impl WarrantRegistry {
    /// Look up a warrant by source basename (case-insensitive).
    #[must_use]
    pub fn find(&self, source_basename: &str) -> Option<&WarrantEntry> {
        let lower = source_basename.to_lowercase();
        self.entries
            .iter()
            .find(|e| e.source.to_lowercase() == lower)
    }
}

/// Validate that a `CommandExit` command contains no mutating verbs.
///
/// # Errors
///
/// Returns a descriptive error if a mutating verb is found.
pub fn validate_command(cmd: &str) -> Result<(), String> {
    // Check the command starts with rm (no leading space)
    if cmd.trim_start().starts_with("rm ") || cmd.trim() == "rm" {
        return Err(format!(
            "CommandExit cmd {cmd:?} contains mutating verb 'rm'"
        ));
    }
    for verb in MUTATING_VERBS {
        if cmd.contains(verb) {
            return Err(format!(
                "CommandExit cmd {cmd:?} contains mutating verb {verb:?}"
            ));
        }
    }
    Ok(())
}

/// Errors that can occur when loading a registry.
#[derive(Debug)]
pub enum RegistryError {
    /// The registry file could not be read.
    Io(std::io::Error),
    /// The registry file could not be parsed.
    Parse(toml::de::Error),
    /// A `CommandExit` assertion contains a mutating verb.
    MutatingCommand(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "registry I/O error: {e}"),
            Self::Parse(e) => write!(f, "registry parse error: {e}"),
            Self::MutatingCommand(msg) => write!(f, "registry safety error: {msg}"),
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::MutatingCommand(_) => None,
        }
    }
}

/// Loader for `warrants.toml` registry files.
pub struct RegistryLoader;

impl RegistryLoader {
    /// Load a registry from the given TOML file path.
    ///
    /// # Errors
    ///
    /// Returns `RegistryError` if the file cannot be read, parsed, or contains
    /// mutating `CommandExit` commands.
    pub fn load(path: &Path) -> Result<WarrantRegistry, RegistryError> {
        let text = std::fs::read_to_string(path).map_err(RegistryError::Io)?;
        Self::load_str(&text)
    }

    /// Load a registry from a TOML string (for tests).
    ///
    /// # Errors
    ///
    /// Returns `RegistryError::Parse` if the TOML is invalid, or
    /// `RegistryError::MutatingCommand` if a command contains a mutating verb.
    pub fn load_str(toml_text: &str) -> Result<WarrantRegistry, RegistryError> {
        let registry: WarrantRegistry =
            toml::from_str(toml_text).map_err(RegistryError::Parse)?;

        // Validate all CommandExit assertions
        for entry in &registry.entries {
            if let AssertionSpec::CommandExit { cmd, .. } = &entry.assertion {
                validate_command(cmd).map_err(RegistryError::MutatingCommand)?;
            }
        }

        Ok(registry)
    }

    /// Return a default registry path relative to a warrants.toml file.
    #[must_use]
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        PathBuf::from(format!("{home}/wintermute/warrant/warrants.toml"))
    }

    /// Return the built-in seed registry as a TOML string.
    #[must_use]
    pub const fn seed_toml() -> &'static str {
        include_str!("../warrants.toml")
    }
}
