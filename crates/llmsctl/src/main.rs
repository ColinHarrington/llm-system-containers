//! `llmsctl` — manage the platform: the L1 VM and services.
//!
//! M1: `up`/`down`/`status` drive the Lima VM via `llmsc-core`. `init` prints a default config.
//! Services (M5) and full config loading (M2) come later.

use clap::{Parser, Subcommand};
use llmsc_core::bootstrap::IncusBootstrap;
use llmsc_core::config::{user_config_path, Config};
use llmsc_core::deploy::LiteLlmDeployer;
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
    /// Per-agent LLM virtual keys (the credential-isolation ring).
    Keys {
        #[command(subcommand)]
        action: KeysAction,
    },
    /// Per-agent Tetragon kernel policies (the per-UID enforcement ring).
    Tetragon {
        /// Sandbox name.
        sandbox: String,
        /// Load the compiled policies into the VM (requires Tetragon installed).
        #[arg(long)]
        apply: bool,
    },
    /// One-shot platform health report: VM, services, config, per-sandbox enforcement.
    Doctor,
}

#[derive(Subcommand)]
enum KeysAction {
    /// List the per-agent virtual keys compiled from guardrails.
    Ls,
    /// Mint/refresh the compiled keys against the running LiteLLM proxy.
    Sync,
    /// Set the upstream provider API key (stored only in the LiteLLM container).
    SetProvider {
        /// Provider (openai | anthropic).
        provider: String,
        /// The provider API key.
        key: String,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// List available services and whether they're enabled.
    List,
    /// Show the live container state of each service (running / stopped / not-provisioned).
    Status,
    /// Enable a service (records it in the user config).
    Enable { name: String },
    /// Disable a service.
    Disable { name: String },
    /// Provision/start enabled services in the VM.
    Up,
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
        ServiceAction::Status => {
            let cfg = Config::load_effective().map_err(|e| e.to_string())?;
            let incus = llmsc_core::incus::CliIncus::new(cfg.vm.name.clone(), &SystemRunner);
            for e in catalog() {
                println!("{:<11} {}", e.name, incus.service_status(e.name).id());
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
        ServiceAction::Up => {
            let cfg = Config::load_effective().map_err(|e| e.to_string())?;
            let vm = cfg.vm.name.clone();
            if cfg.services.is_empty() {
                println!("no services enabled (use `llmsctl services enable <name>`)");
            }
            for svc in &cfg.services {
                match svc.name.as_str() {
                    "litellm" => LiteLlmDeployer::new(vm.clone(), &SystemRunner)
                        .deploy(&ConsoleReporter)
                        .map_err(|e| e.to_string())?,
                    "mitmproxy" => {
                        let d =
                            llmsc_core::deploy::MitmproxyDeployer::new(vm.clone(), &SystemRunner);
                        d.deploy(&ConsoleReporter).map_err(|e| e.to_string())?;
                        d.sync_allowlist(
                            &llmsc_core::enforce::mitmproxy_allowlist(&cfg),
                            &ConsoleReporter,
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    "phoenix" => {
                        llmsc_core::deploy::PhoenixDeployer::new(vm.clone(), &SystemRunner)
                            .deploy(&ConsoleReporter)
                            .map_err(|e| e.to_string())?;
                    }
                    "grafana" => {
                        llmsc_core::deploy::GrafanaStackDeployer::new(vm.clone(), &SystemRunner)
                            .deploy(&ConsoleReporter)
                            .map_err(|e| e.to_string())?;
                    }
                    "seaweedfs" => {
                        llmsc_core::deploy::SeaweedFsDeployer::new(vm.clone(), &SystemRunner)
                            .deploy(&ConsoleReporter)
                            .map_err(|e| e.to_string())?;
                    }
                    other => eprintln!("→ no deployer yet for '{other}' (M5 follow-up)"),
                }
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
        Command::Keys { action } => keys(action)?,
        Command::Tetragon { sandbox, apply } => tetragon(sandbox, apply)?,
        Command::Doctor => doctor()?,
    }
    Ok(())
}

fn doctor() -> Result<(), String> {
    use llmsc_core::incus::CliIncus;
    let cfg = Config::load_effective().unwrap_or_default();
    let vm_name = cfg.vm.name.clone();

    println!("llmsc doctor");
    println!("============");

    let vm_state = LimaVmDriver::new(cfg.vm.clone(), SystemRunner)
        .status()
        .map(|s| format!("{s:?}"))
        .unwrap_or_else(|e| format!("error: {e}"));
    let running = vm_state == "Running";
    println!("\nVM '{vm_name}': {vm_state}");

    println!("\nConfig ({}):", user_config_path().display());
    println!("  operator:         {}", cfg.operator);
    println!("  sandboxes:        {}", cfg.sandboxes.len());
    println!("  services enabled: {}", cfg.services.len());
    println!("  incus profiles:   {}", cfg.incus_profiles.len());

    println!("\nServices:");
    if running {
        let incus = CliIncus::new(vm_name.clone(), &SystemRunner);
        for e in catalog() {
            let enabled = if cfg.service_enabled(e.name) {
                "enabled"
            } else {
                "disabled"
            };
            println!(
                "  {:<11} {:<16} {enabled}",
                e.name,
                incus.service_status(e.name).id()
            );
        }
    } else {
        println!("  (VM not running — skipping live service checks)");
    }

    println!("\nSandboxes (configured enforcement intent):");
    if cfg.sandboxes.is_empty() {
        println!("  (none)");
    }
    for sb in &cfg.sandboxes {
        let s = llmsc_core::enforce::sandbox_enforcement(sb);
        println!(
            "  {:<20} egress={} domains={} agents={} ro-fs={} control-plane={}",
            sb.name,
            s.egress_posture,
            s.domains,
            s.agents,
            s.read_only_agents,
            s.control_plane_agents
        );
    }
    println!("\n(run `llmsc egress <name>` for live ACL state; the GUI Enforcement panel shows live rings)");
    Ok(())
}

fn keys(action: KeysAction) -> Result<(), String> {
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let specs = llmsc_core::enforce::virtual_key_specs(&cfg);
    match action {
        KeysAction::Ls => {
            if specs.is_empty() {
                println!("(no agents with virtual keys)");
            }
            for s in &specs {
                println!(
                    "{:<40} {} @ {:<16} ${:.0}/{}",
                    s.key_alias, s.agent, s.sandbox, s.max_budget_usd, s.budget_duration
                );
            }
        }
        KeysAction::Sync => {
            if specs.is_empty() {
                println!("no agent keys to sync");
                return Ok(());
            }
            let synced = LiteLlmDeployer::new(cfg.vm.name.clone(), &SystemRunner)
                .sync_virtual_keys(&specs, &ConsoleReporter)
                .map_err(|e| e.to_string())?;
            println!("synced {} virtual key(s)", synced.len());
        }
        KeysAction::SetProvider { provider, key } => {
            LiteLlmDeployer::new(cfg.vm.name.clone(), &SystemRunner)
                .set_provider_key(&provider, &key, &ConsoleReporter)
                .map_err(|e| e.to_string())?;
            println!("provider key set (stored only in the LiteLLM container)");
        }
    }
    Ok(())
}

fn tetragon(sandbox: String, apply: bool) -> Result<(), String> {
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let pols = llmsc_core::tetragon::sandbox_policies(&cfg, &sandbox);
    if pols.is_empty() {
        println!("no agents in '{sandbox}' — nothing to enforce at the kernel");
        return Ok(());
    }
    if apply {
        let applied = llmsc_core::deploy::TetragonDeployer::new(cfg.vm.name.clone(), &SystemRunner)
            .apply_policies(&pols, &ConsoleReporter)
            .map_err(|e| e.to_string())?;
        println!("loaded {} Tetragon policy(ies)", applied.len());
    } else {
        for p in &pols {
            println!(
                "{:<40} {} — {} syscalls denied (egress: {})",
                p.name,
                p.agent,
                p.denied_syscalls.len(),
                p.egress_note
            );
        }
        println!("(DRAFT — use --apply to load; requires Tetragon installed in the VM)");
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
