//! warrant — close-claim corpus and assertion runner.
//!
//! ## Usage
//!
//! ```text
//! warrant list [--format table|json]
//! warrant list-sources
//! ```

use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use warrant_core::{classify, AuditPlan, FakeSource, CloseSource};

/// Output format selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
enum Format {
    /// Human-readable table output.
    #[default]
    Table,
    /// Machine-readable JSON output.
    Json,
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
            // Only FakeSource is available in v0.1; real sources arrive in warrant-audit
            let stdout = io::stdout();
            let mut out = stdout.lock();
            writeln!(out, "Available `CloseSource` implementations:")?;
            writeln!(out, "  FakeSource  (in-memory illustrative fixtures, v0.1)")?;
            writeln!(out, "  [Real filesystem source ships in warrant-audit]")?;
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
