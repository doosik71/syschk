//! 진입점. 서브커맨드가 없으면 TUI 를 띄운다.

use clap::Parser;
use syschk::cli::{Cli, Command};
use syschk::{app, commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let code = match cli.command.unwrap_or(Command::Tui) {
        Command::Tui => {
            app::runtime::run()?;
            0
        }
        Command::Doctor { bundle, missing } => commands::doctor::run(bundle.as_deref(), missing),
        Command::Tasks {
            query,
            screen,
            markdown,
        } => commands::tasks::run(query.as_deref(), screen, markdown),
        Command::Check => commands::check::run(),
        Command::Policy => commands::policy::run(),
    };
    std::process::exit(code);
}
