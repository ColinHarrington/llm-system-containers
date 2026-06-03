//! `llmsctl` — manage the platform: the L1 VM and services.
//!
//! M1: `up`/`down`/`status` drive the Lima VM via `llmsc-core`. `init` prints a default config.
//! Services (M5) and full config loading (M2) come later.

use clap::{Parser, Subcommand};
use llmsc_core::bootstrap::IncusBootstrap;
use llmsc_core::config::{user_config_path, Config};
use llmsc_core::process::SystemRunner;
use llmsc_core::progress::Reporter;
use llmsc_core::service::{catalog, lookup};
use llmsc_core::vm::{LimaVmDriver, VmDriver};

/// Prints each step to stderr so progress shows during long operations.
struct ConsoleReporter;

impl Reporter for ConsoleReporter {
    fn step(&self, msg: &str) {
        eprintln!("→ {msg}");
    }
}

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
    /// Start the VM (create it if needed) and bootstrap Incus.
    Up,
    /// Stop the VM.
    Down,
    /// Stop and delete the VM.
    Destroy,
    /// Show VM status.
    Status,
    /// Manage services (LLM proxy, observability, …).
    Services {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// List available services and whether they're enabled.
    List,
    /// Enable a service (records it in the user config).
    Enable { name: String },
    /// Disable a service.
    Disable { name: String },
}

fn services(action: ServiceAction) -> Result<(), String> {
    match action {
        ServiceAction::List => {
            let cfg = Config::load_effective().map_err(|e| e.to_string())?;
            for e in catalog() {
                let mark = if cfg.service_enabled(e.name) {
                    "x"
                } else {
                    " "
                };
                println!(
                    "[{mark}] {:<11} {:<9} {}",
                    e.name, e.priority, e.description
                );
            }
        }
        ServiceAction::Enable { name } => {
            let entry = lookup(&name).ok_or_else(|| format!("unknown service '{name}'"))?;
            let path = user_config_path();
            let mut cfg = if path.exists() {
                Config::load(&path).map_err(|e| e.to_string())?
            } else {
                Config::default()
            };
            if cfg.enable_service(&name, entry.default_placement) {
                cfg.save(&path).map_err(|e| e.to_string())?;
                println!(
                    "enabled '{name}' ({:?}) in {}",
                    entry.default_placement,
                    path.display()
                );
                eprintln!("→ note: provisioning the service container is an M5 follow-up");
            } else {
                println!("'{name}' is already enabled");
            }
        }
        ServiceAction::Disable { name } => {
            let path = user_config_path();
            let mut cfg = if path.exists() {
                Config::load(&path).map_err(|e| e.to_string())?
            } else {
                Config::default()
            };
            if cfg.disable_service(&name) {
                cfg.save(&path).map_err(|e| e.to_string())?;
                println!("disabled '{name}'");
            } else {
                println!("'{name}' was not enabled");
            }
        }
    }
    Ok(())
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
            let cfg = Config::default();
            let vm_name = cfg.vm.name.clone();
            LimaVmDriver::new(cfg.vm, SystemRunner)
                .up(&ConsoleReporter)
                .map_err(|e| e.to_string())?;
            IncusBootstrap::new(vm_name, &SystemRunner)
                .run(&ConsoleReporter)
                .map_err(|e| e.to_string())?;
            println!("VM is up with Incus ready");
        }
        Command::Down => {
            driver().down().map_err(|e| e.to_string())?;
            println!("VM stopped");
        }
        Command::Destroy => {
            driver().destroy().map_err(|e| e.to_string())?;
            println!("VM destroyed");
        }
        Command::Status => {
            let status = driver().status().map_err(|e| e.to_string())?;
            println!("VM: {status:?}");
        }
        Command::Services { action } => services(action)?,
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
