//! 명령행 인터페이스.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "syschk",
    version,
    about = "Guided, read-only system diagnosis for Ubuntu",
    long_about = "syschk helps you understand what a machine is doing and why it failed.\n\
                  It only reads: it never changes configuration, restarts services or\n\
                  installs packages. Run it with no arguments for the interactive view."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start the interactive terminal interface (default).
    Tui,
    /// Report which diagnosis tools are present, and how to get the missing ones.
    Doctor {
        /// Show only one bundle: core, storage, network, hardware, diagnostics, updates, containers, advanced.
        #[arg(long)]
        bundle: Option<String>,
        /// List only what is missing.
        #[arg(long)]
        missing: bool,
    },
    /// List what syschk can look into.
    Tasks {
        /// Filter by symptom wording, e.g. "disk full".
        query: Option<String>,
        /// Filter by screen number (1-14).
        #[arg(long)]
        screen: Option<usize>,
        /// Emit Markdown tables (used to generate the docs).
        #[arg(long)]
        markdown: bool,
    },
    /// One-shot summary for scripts and cron.
    Check,
    /// Print the read-only command policy that constrains every probe.
    Policy,
}
