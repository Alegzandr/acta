//! `acta`: console executable. Packaged as `acta-cli`, exported to the user
//! as `acta` by the GUI (ADR 0004). M1 only knows its own version.

use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "acta",
    bin_name = "acta",
    version = acta_core::VERSION,
    about = "Acta: project tracker and scoped memory for Claude Code",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print the version
    Version,
}

fn main() -> ExitCode {
    match Cli::try_parse() {
        Ok(Cli {
            command: Some(Command::Version),
        }) => {
            println!("{}", version_line());
            ExitCode::SUCCESS
        }
        Ok(Cli { command: None }) => {
            // Nothing asked: show what exists.
            let _ = Cli::command().print_help();
            ExitCode::SUCCESS
        }
        Err(err) if is_help_or_version(&err) => {
            let _ = err.print();
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{}", one_line(&err.to_string()));
            ExitCode::from(2)
        }
    }
}

fn is_help_or_version(err: &clap::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    )
}

fn version_line() -> String {
    format!("acta {}", acta_core::VERSION)
}

/// clap renders an error as several lines (message, usage, hint). Agents and
/// hooks read this output, so keep the message and point at `--help`.
fn one_line(rendered: &str) -> String {
    let first = rendered.lines().next().unwrap_or("invalid arguments");
    let message = first.strip_prefix("error: ").unwrap_or(first);
    format!("acta: {message}. Run 'acta --help' for the commands.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_line_is_name_and_version() {
        assert_eq!(
            version_line(),
            format!("acta {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn version_flag_prints_the_same_line_as_the_subcommand() {
        let err = Cli::try_parse_from(["acta", "--version"]).err().unwrap();
        assert!(is_help_or_version(&err));
        assert_eq!(err.to_string().trim_end(), version_line());
    }

    #[test]
    fn version_subcommand_parses() {
        let cli = Cli::try_parse_from(["acta", "version"]).ok().unwrap();
        assert!(matches!(cli.command, Some(Command::Version)));
    }

    #[test]
    fn unknown_command_is_reported_on_one_line() {
        let err = Cli::try_parse_from(["acta", "frobnicate"]).err().unwrap();
        assert!(!is_help_or_version(&err));
        let line = one_line(&err.to_string());
        assert_eq!(line.lines().count(), 1);
        assert!(line.contains("frobnicate"), "{line}");
    }

    #[test]
    fn clap_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
