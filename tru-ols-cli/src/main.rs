use anyhow::Result;
use clap::{ArgAction, Parser};
use tracing_subscriber::EnvFilter;
use tru_ols::commands::{self, run_command};

/// TRU-OLS - Truncated ReUnmixing OLS for Flow Cytometry
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "tru-ols")]
#[command(about = "TRU-OLS unmixing for flow cytometry FCS files", long_about = None)]
#[command(subcommand_required = false)]
struct Cli {
    /// Less tracing output (caps log level at `warn`; `--quiet` wins over `RUST_LOG` and `--verbose`).
    /// Does not affect interactive prompts. The one-line startup banner still prints.
    #[arg(short = 'q', long, global = true, action = ArgAction::SetTrue)]
    quiet: bool,

    /// Show `info` tracing during interactive mode (`tru-ols` with no subcommand, or
    /// `tru-ols interactive`). By default interactive mode uses `warn` unless `RUST_LOG` is set.
    /// Ignored when combined with `--quiet`.
    #[arg(short = 'v', long, global = true, action = ArgAction::SetTrue)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<commands::Command>,
}

fn interactive_like(command: &Option<commands::Command>) -> bool {
    matches!(command, None | Some(commands::Command::Interactive))
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let filter = if args.quiet {
        EnvFilter::new("warn")
    } else if interactive_like(&args.command) && !args.verbose {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    println!(
        "🧬 TRU-OLS - Flow Cytometry Unmixing v{}",
        env!("CARGO_PKG_VERSION")
    );
    println!("============================================\n");

    run_command(args.command.as_ref())?;

    Ok(())
}
