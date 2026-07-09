use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use harness_bench::artifact::Artifact;
use harness_bench::responsibility::rollup;
use harness_bench::score::score_artifact;
use harness_bench::task::TaskSpec;

#[derive(Parser)]
#[command(name = "harness-bench", about = "Benchmark harness scoring engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Score a single captured artifact against its task spec.
    Score {
        /// Path to the captured artifact directory.
        #[arg(long)]
        artifact: PathBuf,
        /// Path to the task's expected.toml spec.
        #[arg(long)]
        task: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Score { artifact, task } => match run_score(&artifact, &task) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
    }
}

fn run_score(
    artifact_dir: &Path,
    task_path: &Path,
) -> Result<(), harness_bench::error::BenchError> {
    let spec = TaskSpec::load(task_path)?;
    let artifact = Artifact::load(artifact_dir)?;
    let score = score_artifact(&spec, &artifact)?;

    println!("Task {} | arm {} | k {}", score.task, score.arm, score.k);
    println!(
        "  functional: {}",
        if score.functional { "PASS" } else { "FAIL" }
    );
    println!("  checks:");
    for c in &score.checks {
        println!(
            "    [{}] {} ({}): {}",
            if c.passed { "PASS" } else { "FAIL" },
            c.id,
            c.responsibility,
            c.detail
        );
    }
    println!("  responsibility rollup:");
    for (responsibility, tally) in rollup(&score.checks) {
        println!(
            "    {:<24} {}/{}",
            responsibility, tally.passed, tally.total
        );
    }
    Ok(())
}
