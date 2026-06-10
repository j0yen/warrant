//! warrant — close-claim corpus and assertion runner.
//!
//! ## Usage
//!
//! ```text
//! warrant list [--format table|json]
//! warrant list-sources
//! warrant audit [--archive <dir>] [--registry <toml>] [--format json] [--status refuted|unwarranted|all]
//! warrant report [--apply] [--registry <toml>] [--archive <dir>]
//! ```

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use warrant_audit::runner::StatusFilter;
use warrant_audit::{run_audit, FsDocketSource, RegistryLoader};
use warrant_core::{classify, default_state_path, AuditPlan, CloseSource, FakeSource, PrintDocketSink};
use warrant_docket::report_verdicts;

/// Output format selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
enum Format {
    /// Human-readable table output.
    #[default]
    Table,
    /// Machine-readable JSON output.
    Json,
}

/// Status filter for `warrant audit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
enum AuditStatus {
    /// Show all verdicts.
    #[default]
    All,
    /// Show only `Refuted` verdicts.
    Refuted,
    /// Show only `Unwarranted` verdicts.
    Unwarranted,
}

impl From<AuditStatus> for StatusFilter {
    fn from(s: AuditStatus) -> Self {
        match s {
            AuditStatus::All => Self::All,
            AuditStatus::Refuted => Self::Refuted,
            AuditStatus::Unwarranted => Self::Unwarranted,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "warrant",
    version,
    about = "Close-claim corpus and assertion runner for the warrant suite"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List and classify close claims from the default corpus source.
    List {
        /// Output format (table or json).
        #[arg(long, value_enum, default_value_t = Format::Table)]
        format: Format,
    },
    /// List available `CloseSource` implementations.
    ListSources,
    /// Run the assertion runner over a close-claim archive and registry.
    Audit {
        /// Path to the PRD archive directory.
        #[arg(long)]
        archive: Option<PathBuf>,
        /// Path to `warrants.toml` registry file.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Table)]
        format: Format,
        /// Filter by verdict status.
        #[arg(long, value_enum, default_value_t = AuditStatus::All)]
        status: AuditStatus,
    },
    /// Report `Refuted` verdicts to the docket ledger (edge-triggered).
    Report {
        /// Actually perform docket reopens (default: print-only).
        #[arg(long)]
        apply: bool,
        /// Path to `warrants.toml` registry file.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Path to the PRD archive directory.
        #[arg(long)]
        archive: Option<PathBuf>,
        /// Path to the persisted reported-state file.
        #[arg(long)]
        state: Option<PathBuf>,
    },
}

fn print_table(plan: &AuditPlan) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    writeln!(out, "{:<60} {:<20} {:<14}", "Source", "Kind", "Date")?;
    writeln!(out, "{}", "-".repeat(96))?;

    for claim in &plan.claims {
        let date = claim.close_date.as_deref().unwrap_or("-");
        writeln!(
            out,
            "{:<60} {:<20} {:<14}",
            truncate(&claim.source.to_string(), 58),
            claim.kind.to_string(),
            date
        )?;
        if let Some(mech) = &claim.mechanism_text {
            writeln!(out, "  mechanism: {}", truncate(mech, 80))?;
        }
    }

    writeln!(out)?;
    writeln!(out, "Tally:")?;
    writeln!(out, "  AcsMet:             {}", plan.tally.acs_met)?;
    writeln!(out, "  MechanismAsserted:  {}", plan.tally.mechanism_asserted)?;
    writeln!(out, "  Superseded:         {}", plan.tally.superseded)?;
    writeln!(out, "  LiveDeferred:       {}", plan.tally.live_deferred)?;
    writeln!(out, "  Unclassified:       {}", plan.tally.unclassified)?;
    writeln!(out, "  Total:              {}", plan.tally.total())?;

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::List { format } => {
            let source = FakeSource::with_illustrative_fixtures();
            let raw = source.close_notes()?;
            let plan = classify(&raw);

            match format {
                Format::Table => {
                    print_table(&plan)?;
                }
                Format::Json => {
                    #[allow(clippy::print_stdout)]
                    {
                        let json = serde_json::to_string_pretty(&plan)?;
                        println!("{json}");
                    }
                }
            }
        }

        Commands::ListSources => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            writeln!(out, "Available `CloseSource` implementations:")?;
            writeln!(out, "  FakeSource         (in-memory illustrative fixtures)")?;
            writeln!(out, "  FsDocketSource     (filesystem archive, warrant-audit)")?;
        }

        Commands::Audit {
            archive,
            registry,
            format,
            status,
        } => {
            let source: Box<dyn CloseSource> = match archive {
                Some(dir) => Box::new(FsDocketSource::new(dir)),
                None => Box::new(FsDocketSource::default_live()),
            };

            let reg = match registry {
                Some(path) => RegistryLoader::load(&path)?,
                None => {
                    let default_path = RegistryLoader::default_path();
                    if default_path.exists() {
                        RegistryLoader::load(&default_path)?
                    } else {
                        // Fall back to built-in seed registry
                        RegistryLoader::load_str(RegistryLoader::seed_toml())?
                    }
                }
            };

            let filter: StatusFilter = status.into();
            let result = run_audit(source.as_ref(), &reg, filter)?;

            match format {
                Format::Table => {
                    let stdout = io::stdout();
                    let mut out = stdout.lock();
                    writeln!(
                        out,
                        "{:<50} {:<20} {:<14} {}",
                        "Source", "Kind", "Status", "Evidence"
                    )?;
                    writeln!(out, "{}", "-".repeat(110))?;
                    for v in &result.verdicts {
                        writeln!(
                            out,
                            "{:<50} {:<20} {:<14} {}",
                            truncate(&v.source.to_string(), 48),
                            format!("{}", v.kind),
                            format!("{:?}", v.status),
                            truncate(&v.evidence, 60),
                        )?;
                    }
                    writeln!(out)?;
                    writeln!(out, "Tally:")?;
                    writeln!(out, "  Proven:       {}", result.tally.proven)?;
                    writeln!(out, "  Refuted:      {}", result.tally.refuted)?;
                    writeln!(out, "  Unproven:     {}", result.tally.unproven)?;
                    writeln!(out, "  Unwarranted:  {}", result.tally.unwarranted)?;
                    writeln!(out, "  Total:        {}", result.tally.total())?;
                }
                Format::Json => {
                    #[allow(clippy::print_stdout)]
                    {
                        let json = serde_json::to_string_pretty(&result)?;
                        println!("{json}");
                    }
                }
            }
        }

        Commands::Report {
            apply,
            registry,
            archive,
            state,
        } => {
            let source: Box<dyn CloseSource> = match archive {
                Some(dir) => Box::new(FsDocketSource::new(dir)),
                None => Box::new(FsDocketSource::default_live()),
            };

            let reg = match registry {
                Some(path) => RegistryLoader::load(&path)?,
                None => {
                    let default_path = RegistryLoader::default_path();
                    if default_path.exists() {
                        RegistryLoader::load(&default_path)?
                    } else {
                        RegistryLoader::load_str(RegistryLoader::seed_toml())?
                    }
                }
            };

            // Run audit to get verdicts, then report Refuted ones
            let audit_result = run_audit(source.as_ref(), &reg, StatusFilter::All)?;

            let state_path = state.unwrap_or_else(default_state_path);
            let sink = PrintDocketSink;
            let report = report_verdicts(&audit_result.verdicts, &state_path, &sink, apply)?;

            let stdout = io::stdout();
            let mut out = stdout.lock();

            if !apply {
                writeln!(out, "warrant report (dry-run — use --apply to perform reopens)")?;
                writeln!(out)?;
            }

            for entry in &report.entries {
                match entry.action {
                    warrant_docket::ReportAction::WouldReopen => {
                        let prefix = if apply { "[reopened]" } else { "[would reopen]" };
                        writeln!(out, "{prefix} {}", entry.slug)?;
                        writeln!(out, "  summary:  {}", entry.summary)?;
                        writeln!(out, "  evidence: {}", truncate(&entry.evidence, 100))?;
                    }
                    warrant_docket::ReportAction::AlreadyReported => {
                        writeln!(out, "[already reported] {} (no-op)", entry.slug)?;
                    }
                    warrant_docket::ReportAction::AutoClosed => {
                        writeln!(out, "[auto-closed] {} (verdict now Proven)", entry.slug)?;
                    }
                    warrant_docket::ReportAction::NoAction => {}
                }
            }

            writeln!(out)?;
            writeln!(out, "reopened: {}  auto-closed: {}", report.reopened, report.auto_closed)?;
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    sigpipe::reset();
    #[allow(clippy::print_stderr)]
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("warrant: error: {e}");
            ExitCode::FAILURE
        }
    }
}
