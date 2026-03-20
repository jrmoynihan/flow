mod commands;
mod config;
mod interactive;
mod output;
mod synthetic_data;

use anyhow::Result;
use clap::Parser;
use commands::run_command;
use tracing_subscriber::EnvFilter;

/// TRU-OLS - Truncated ReUnmixing OLS for Flow Cytometry
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "tru-ols")]
#[command(about = "TRU-OLS unmixing for flow cytometry FCS files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: commands::Command,
}

fn main() -> Result<()> {
    // Initialize tracing subscriber
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let args = Cli::parse();

    println!(
        "🧬 TRU-OLS - Flow Cytometry Unmixing v{}",
        env!("CARGO_PKG_VERSION")
    );
    println!("============================================\n");

    run_command(&args.command)?;

    Ok(())
}
