use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::application::{
    ApplicationError, CoreApplication, CoreDistributionPort, InstallationStatePort,
    ThreeWayMergePort,
};
use crate::interface::presenter::{
    present_doctor, present_install, present_status, present_update, CommandExit,
};

#[derive(Debug, Parser)]
#[command(
    name = "harness",
    version,
    about = "Install and safely maintain a repository Harness core"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Install a fresh core or adopt an existing copy-on-install core.
    Install {
        #[arg(long, default_value = ".")]
        directory: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Preview or apply a conflict-safe three-way core update.
    Update {
        #[arg(long, default_value = ".")]
        directory: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Inspect installed version and consumer modifications without mutation.
    Status {
        #[arg(long, default_value = ".")]
        directory: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Validate provenance, paths, merge support, and transaction health.
    Doctor {
        #[arg(long, default_value = ".")]
        directory: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

pub fn execute<D, S, M>(cli: Cli, application: &CoreApplication<D, S, M>) -> CommandExit
where
    D: CoreDistributionPort,
    S: InstallationStatePort,
    M: ThreeWayMergePort,
{
    let result = match cli.command {
        Command::Install {
            directory,
            dry_run,
            json,
        } => application
            .install(&directory, dry_run)
            .map(|report| present_install(&report, json)),
        Command::Update {
            directory,
            dry_run,
            json,
        } => application
            .update(&directory, dry_run)
            .map(|report| present_update(&report, json)),
        Command::Status { directory, json } => application
            .status(&directory)
            .map(|report| present_status(&report, json)),
        Command::Doctor { directory, json } => application
            .doctor(&directory)
            .map(|report| present_doctor(&report, json)),
    };
    result.unwrap_or_else(present_error)
}

fn present_error(error: ApplicationError) -> CommandExit {
    CommandExit {
        code: 1,
        stdout: String::new(),
        stderr: format!("Error: {error}\n"),
    }
}
