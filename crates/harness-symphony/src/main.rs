mod agent;
mod auto;
mod changeset;
mod config;
mod doctor;
mod interface;
mod pr;
mod retention;
mod run;
mod state;
mod sync;
mod upload;
mod web;
mod work;

use clap::Parser;

fn main() {
    let cli = interface::Cli::parse();
    if let Err(error) = interface::run(cli) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
