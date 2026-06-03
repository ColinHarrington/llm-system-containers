//! `llmsc` — manage individual LLM System Containers (L2 sandboxes).
//! Skeleton (M0); behavior arrives in M2 (lifecycle) and M6 (cp).

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "llmsc",
    about = "Manage LLM System Containers (L2 sandboxes)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create and start a sandbox.
    Launch { name: String },
    /// List sandboxes.
    Ls,
    /// Open a shell as `user@sandbox`.
    Shell { target: String },
    /// Copy files (host↔container, container↔container).
    Cp { src: String, dst: String },
    /// Remove a sandbox.
    Rm { name: String },
}

fn main() {
    match Cli::parse().command {
        Command::Launch { name } => println!("launch {name}: not yet implemented (M2)"),
        Command::Ls => println!("ls: not yet implemented (M2)"),
        Command::Shell { target } => println!("shell {target}: not yet implemented (M2)"),
        Command::Cp { src, dst } => println!("cp {src} -> {dst}: not yet implemented (M6)"),
        Command::Rm { name } => println!("rm {name}: not yet implemented (M2)"),
    }
}
