//! `llmsctl` — manage the platform: the L1 VM and services.
//! Skeleton (M0); behavior arrives in M1 (VM bring-up) and M5 (services).

use clap::{Parser, Subcommand};

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
    /// Run the setup wizard and write config.
    Init,
    /// Start the VM.
    Up,
    /// Stop the VM.
    Down,
    /// Show VM status.
    Status,
}

fn main() {
    match Cli::parse().command {
        Command::Init => println!("init: not yet implemented (M1)"),
        Command::Up => println!("up: not yet implemented (M1)"),
        Command::Down => println!("down: not yet implemented (M1)"),
        Command::Status => println!("status: not yet implemented (M1)"),
    }
}
