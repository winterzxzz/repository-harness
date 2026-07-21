use std::io::{self, Write};

use clap::Parser;
use harness::application::CoreApplication;
use harness::infrastructure::{
    EmbeddedCoreDistribution, FileSystemInstallationState, GitThreeWayMerge,
};
use harness::interface::{execute, Cli};

fn main() {
    let cli = Cli::parse();
    let application = CoreApplication::new(
        EmbeddedCoreDistribution,
        FileSystemInstallationState,
        GitThreeWayMerge,
    );
    let exit = execute(cli, &application);
    if !exit.stdout.is_empty() {
        let _ = io::stdout().write_all(exit.stdout.as_bytes());
    }
    if !exit.stderr.is_empty() {
        let _ = io::stderr().write_all(exit.stderr.as_bytes());
    }
    std::process::exit(exit.code);
}
