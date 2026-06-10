//! `FsDocketSource` — reads closed PRDs from a filesystem archive directory.

use std::path::{Path, PathBuf};

use warrant_core::{CloseRef, CloseSource, RawClose};

/// A `CloseSource` backed by a filesystem directory of PRD markdown files.
///
/// Reads all `*.md` files in the archive directory that contain a
/// `Status: Closed` line. If a `docket` binary/store is present it is also
/// consulted, but its absence is silently ignored (fail-open).
#[derive(Debug, Clone)]
pub struct FsDocketSource {
    archive_dir: PathBuf,
}

impl FsDocketSource {
    /// Create a new `FsDocketSource` reading from the given archive directory.
    #[must_use]
    pub fn new(archive_dir: impl Into<PathBuf>) -> Self {
        Self {
            archive_dir: archive_dir.into(),
        }
    }

    /// Create a `FsDocketSource` pointing at the default live archive.
    #[must_use]
    pub fn default_live() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Self::new(format!("{home}/wintermute/autobuilder/PRDs-archive"))
    }
}

/// Read one markdown file and return a [`RawClose`] if it is a closed PRD.
fn read_closed_prd(path: &Path) -> Option<RawClose> {
    let text = std::fs::read_to_string(path).ok()?;
    let lower = text.to_lowercase();
    if lower.contains("status: closed") {
        Some(RawClose {
            reference: CloseRef::Prd(path.to_path_buf()),
            text,
        })
    } else {
        None
    }
}

impl CloseSource for FsDocketSource {
    fn close_notes(&self) -> Result<Vec<RawClose>, Box<dyn std::error::Error>> {
        let dir = &self.archive_dir;
        if !dir.exists() {
            // Fail-open: absent archive contributes zero rows
            return Ok(Vec::new());
        }

        let mut notes = Vec::new();
        let entries = std::fs::read_dir(dir).map_err(|e| {
            format!("FsDocketSource: cannot read archive dir {}: {e}", dir.display())
        })?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue, // skip unreadable entries
            };
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                if let Some(raw) = read_closed_prd(&path) {
                    notes.push(raw);
                }
            }
        }

        // Sort for deterministic ordering
        notes.sort_by(|a, b| a.reference.to_string().cmp(&b.reference.to_string()));

        Ok(notes)
    }
}
