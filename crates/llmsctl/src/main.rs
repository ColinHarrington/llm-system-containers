//! `llmsctl` — manage the platform: the L1 VM and services.
//!
//! M1: `up`/`down`/`status` drive the Lima VM via `llmsc-core`. `init` prints a default config.
//! Services (M5) and full config loading (M2) come later.

use clap::{Parser, Subcommand};
use llmsc_core::bootstrap::IncusBootstrap;
use llmsc_core::config::{user_config_path, Config, DeploymentMode};
use llmsc_core::deploy::{LiteLlmDeployer, Target};
use llmsc_core::keystore;
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
    /// Show the effective config (target + counts) and validate it.
    Config,
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
    /// Configure Vertex AI as the upstream provider (service-account JSON; creds stay in the
    /// LiteLLM container only). Agents keep using their virtual keys.
    SetVertex {
        /// GCP project id.
        #[arg(long)]
        project: String,
        /// Vertex region, e.g. us-central1.
        #[arg(long, default_value = "us-central1")]
        location: String,
        /// Path to the service-account JSON key file (read on the host; never stored in config).
        #[arg(long)]
        creds: String,
        /// The Vertex model to serve as `default`.
        #[arg(long, default_value = "vertex_ai/gemini-2.0-flash")]
        model: String,
    },
    /// Show per-key spend read back from the proxy.
    Usage,
    /// Rotate an agent's virtual key: mint a fresh token, revoke the old one (re-inject after).
    Rotate {
        /// Target `agent@sandbox`.
        target: String,
    },
    /// Revoke an agent's virtual key on the proxy and forget it locally.
    Revoke {
        /// Target `agent@sandbox`.
        target: String,
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
            let vm = deploy_target(&cfg)?;
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
                    "zeek" => {
                        llmsc_core::deploy::ZeekDeployer::new(vm.clone(), &SystemRunner)
                            .deploy(&ConsoleReporter)
                            .map_err(|e| e.to_string())?;
                    }
                    other => eprintln!("→ no deployer yet for '{other}' (M5 follow-up)"),
                }
            }
            // With both the proxy and Phoenix up, wire LiteLLM's traces to the collector so every
            // virtual-key call is observable (the callback is already in the generated config).
            if let Some(host) = llmsc_core::deploy::phoenix_collector_host(&cfg) {
                LiteLlmDeployer::new(vm.clone(), &SystemRunner)
                    .enable_phoenix(&host, &ConsoleReporter)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Load the effective config and resolve the deployment target — errors on `local`/`remote`,
/// which are reserved and not wired yet (see `Config::vm_target`).
fn effective_cfg() -> Result<Config, String> {
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    cfg.vm_target().map_err(|e| e.to_string())?;
    Ok(cfg)
}

fn driver() -> Result<LimaVmDriver<SystemRunner>, String> {
    Ok(LimaVmDriver::new(effective_cfg()?.vm, SystemRunner))
}

/// The service-deployer transport for the configured deployment target: the VM (`vm`) or the host
/// (`local`). `remote` is reserved.
fn deploy_target(cfg: &Config) -> Result<Target, String> {
    match cfg.mode {
        DeploymentMode::Vm => Ok(Target::Vm(cfg.vm.name.clone())),
        DeploymentMode::Local => Ok(Target::Local),
        DeploymentMode::Remote => {
            Err("deployment target 'remote' is not supported yet (use 'vm' or 'local')".into())
        }
    }
}

fn run() -> Result<(), String> {
    match Cli::parse().command {
        Command::Init => {
            let toml = Config::default().to_toml().map_err(|e| e.to_string())?;
            print!("{toml}");
        }
        Command::Up => {
            let cfg = effective_cfg()?;
            let vm_name = cfg.vm.name.clone();
            LimaVmDriver::new(cfg.vm, SystemRunner)
                .up(&ConsoleReporter)
                .map_err(|e| e.to_string())?;
            IncusBootstrap::new(vm_name.clone(), &SystemRunner)
                .run(&ConsoleReporter)
                .map_err(|e| e.to_string())?;
            // Repair the default profile if `admin init` left it without a root disk / nic —
            // otherwise the first `llmsc launch` fails with "No root device could be found".
            let repaired = llmsc_core::incus::CliIncus::new(vm_name, &SystemRunner)
                .ensure_incus_base(&ConsoleReporter)
                .map_err(|e| e.to_string())?;
            if repaired.is_empty() {
                println!("VM is up with Incus ready");
            } else {
                println!(
                    "VM is up with Incus ready (repaired: {})",
                    repaired.join("; ")
                );
            }
        }
        Command::Down => {
            driver()?.down().map_err(|e| e.to_string())?;
            println!("VM stopped");
        }
        Command::Destroy => {
            driver()?.destroy().map_err(|e| e.to_string())?;
            println!("VM destroyed");
        }
        Command::Status => {
            let status = driver()?.status().map_err(|e| e.to_string())?;
            println!("VM: {status:?}");
        }
        Command::Services { action } => services(action)?,
        Command::Keys { action } => keys(action)?,
        Command::Tetragon { sandbox, apply } => tetragon(sandbox, apply)?,
        Command::Doctor => doctor()?,
        Command::Config => {
            let cfg = Config::load_effective().map_err(|e| e.to_string())?;
            println!("target:    {} (vm '{}')", cfg.mode.id(), cfg.vm.name);
            println!(
                "sandboxes: {}   services: {}",
                cfg.sandboxes.len(),
                cfg.services.len()
            );
            let issues = cfg.validate();
            if issues.is_empty() {
                println!("config valid");
            } else {
                eprintln!("config has {} issue(s):", issues.len());
                for i in &issues {
                    eprintln!("  - {i}");
                }
                return Err("invalid config".to_string());
            }
        }
    }
    Ok(())
}

fn doctor() -> Result<(), String> {
    use llmsc_core::incus::CliIncus;
    let cfg = Config::load_effective().unwrap_or_default();
    let vm_name = cfg.vm.name.clone();

    println!("llmsc doctor");
    println!("============");

    print!("\nTarget: {}", cfg.mode.id());
    if cfg.mode.is_vm() {
        println!(" (VM '{vm_name}')");
    } else {
        println!(" — reserved, not wired yet (only 'vm' is supported)");
    }
    println!("Control surface: Incus API is local-only (not exposed on the network)");

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
            "  {:<20} egress={} domains={} agents={} ro-fs={} control-plane={} filtering={}",
            sb.name,
            s.egress_posture,
            s.domains,
            s.agents,
            s.read_only_agents,
            s.control_plane_agents,
            if sb.net_filtering { "on" } else { "off" }
        );
    }
    println!("\nRemote display:");
    let with_display: Vec<_> = cfg
        .sandboxes
        .iter()
        .filter(|s| !s.display.is_none())
        .collect();
    if with_display.is_empty() {
        println!(
            "  (none — set `display = \"xpra\"|\"x11\"` under [[sandbox]] to surface GUI apps)"
        );
    } else {
        for sb in &with_display {
            println!("  {:<20} {}", sb.name, sb.display.id());
        }
        println!(
            "  host needs: xpra \u{2265} 6 for `xpra` (`xpra --version`); an X server for `x11` (XQuartz on macOS)."
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
            // Reuse already-minted tokens (stable re-sync), then persist the results so they can be
            // injected into agents and rotated later. Tokens are 0600 in the host key store.
            let store_path = keystore::default_key_store_path();
            let mut store = keystore::KeyStore::load(&store_path).map_err(|e| e.to_string())?;
            let minted = LiteLlmDeployer::new(deploy_target(&cfg)?, &SystemRunner)
                .sync_virtual_keys(&specs, store.tokens(), &ConsoleReporter)
                .map_err(|e| e.to_string())?;
            for k in &minted {
                store.upsert(k.alias.clone(), k.token.clone());
            }
            store.save(&store_path).map_err(|e| e.to_string())?;
            println!("synced {} virtual key(s)", minted.len());
        }
        KeysAction::SetProvider { provider, key } => {
            LiteLlmDeployer::new(deploy_target(&cfg)?, &SystemRunner)
                .set_provider_key(&provider, &key, &ConsoleReporter)
                .map_err(|e| e.to_string())?;
            println!("provider key set (stored only in the LiteLLM container)");
        }
        KeysAction::SetVertex {
            project,
            location,
            creds,
            model,
        } => {
            use base64::Engine;
            let bytes = std::fs::read(&creds).map_err(|e| format!("reading {creds}: {e}"))?;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            LiteLlmDeployer::new(deploy_target(&cfg)?, &SystemRunner)
                .set_vertex_provider(&b64, &project, &location, &model, &ConsoleReporter)
                .map_err(|e| e.to_string())?;
            println!("Vertex AI configured (credentials stored only in the LiteLLM container)");
        }
        KeysAction::Usage => {
            let usage = LiteLlmDeployer::new(deploy_target(&cfg)?, &SystemRunner)
                .key_usage()
                .map_err(|e| e.to_string())?;
            if usage.is_empty() {
                println!("(no key usage reported)");
            }
            for u in &usage {
                println!("{:<40} ${:.4}", u.key_alias, u.spend);
            }
        }
        KeysAction::Rotate { target } => {
            let (agent, sandbox) = split_target(&target)?;
            let alias = llmsc_core::enforce::key_alias(&sandbox, &agent);
            let spec = specs
                .iter()
                .find(|s| s.key_alias == alias)
                .ok_or_else(|| format!("'{agent}@{sandbox}' is not a config-managed agent key"))?;
            let store_path = keystore::default_key_store_path();
            let mut store = keystore::KeyStore::load(&store_path).map_err(|e| e.to_string())?;
            let old = store.get(&alias).map(str::to_string);
            let deployer = LiteLlmDeployer::new(deploy_target(&cfg)?, &SystemRunner);
            // Mint a fresh token (empty `existing` → new random suffix), then drop the old one.
            let minted = deployer
                .sync_virtual_keys(
                    std::slice::from_ref(spec),
                    &std::collections::BTreeMap::new(),
                    &ConsoleReporter,
                )
                .map_err(|e| e.to_string())?;
            if let Some(old_token) = old {
                let _ = deployer.delete_key(&old_token, &ConsoleReporter);
            }
            for k in &minted {
                store.upsert(k.alias.clone(), k.token.clone());
            }
            store.save(&store_path).map_err(|e| e.to_string())?;
            println!(
                "rotated {agent}@{sandbox} — run `llmsc agent env {agent}@{sandbox}` to inject the new key"
            );
        }
        KeysAction::Revoke { target } => {
            let (agent, sandbox) = split_target(&target)?;
            let alias = llmsc_core::enforce::key_alias(&sandbox, &agent);
            let store_path = keystore::default_key_store_path();
            let mut store = keystore::KeyStore::load(&store_path).map_err(|e| e.to_string())?;
            match store.get(&alias).map(str::to_string) {
                Some(token) => {
                    LiteLlmDeployer::new(deploy_target(&cfg)?, &SystemRunner)
                        .delete_key(&token, &ConsoleReporter)
                        .map_err(|e| e.to_string())?;
                    store.remove(&alias);
                    store.save(&store_path).map_err(|e| e.to_string())?;
                    println!("revoked {agent}@{sandbox}");
                }
                None => println!("no minted key for {agent}@{sandbox}"),
            }
        }
    }
    Ok(())
}

/// Parse `agent@sandbox` into `(agent, sandbox)`.
fn split_target(t: &str) -> Result<(String, String), String> {
    t.split_once('@')
        .map(|(a, s)| (a.to_string(), s.to_string()))
        .ok_or_else(|| format!("target must be agent@sandbox (got '{t}')"))
}

fn tetragon(sandbox: String, apply: bool) -> Result<(), String> {
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let pols = llmsc_core::tetragon::sandbox_policies(&cfg, &sandbox);
    if pols.is_empty() {
        println!("no agents in '{sandbox}' — nothing to enforce at the kernel");
        return Ok(());
    }
    if apply {
        let applied =
            llmsc_core::deploy::TetragonDeployer::new(deploy_target(&cfg)?, &SystemRunner)
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
