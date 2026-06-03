//! `llmsctl` — manage the platform: the L1 VM and services.
//!
//! M1: `up`/`down`/`status` drive the Lima VM via `llmsc-core`. `init` prints a default config.
//! Services (M5) and full config loading (M2) come later.

use clap::{Parser, Subcommand};
use llmsc_core::config::Config;
use llmsc_core::process::SystemRunner;
use llmsc_core::vm::{LimaVmDriver, VmDriver};

#[derive(Parser)]
#[command(
    name = "llmsctl",
    about = "Manage the llmsc platform (L1 VM + services)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a default config (M2 will write it to disk).
    Init,
    /// Start the VM (create it if needed).
    Up,
    /// Stop the VM.
    Down,
    /// Show VM status.
    Status,
}

fn driver() -> LimaVmDriver<SystemRunner> {
    // M2 will load this from the on-disk config; for now use defaults.
    LimaVmDriver::new(Config::default().vm, SystemRunner)
}

fn run() -> Result<(), String> {
    match Cli::parse().command {
        Command::Init => {
            let toml = Config::default().to_toml().map_err(|e| e.to_string())?;
            print!("{toml}");
        }
        Command::Up => {
            driver().up().map_err(|e| e.to_string())?;
            println!("VM is up");
        }
        Command::Down => {
            driver().down().map_err(|e| e.to_string())?;
            println!("VM stopped");
        }
        Command::Status => {
            let status = driver().status().map_err(|e| e.to_string())?;
            println!("VM: {status:?}");
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
