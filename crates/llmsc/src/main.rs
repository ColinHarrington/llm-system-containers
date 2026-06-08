//! `llmsc` — manage individual LLM System Containers (L2 sandboxes).
//!
//! M2: launch/ls/rm drive Incus (in the VM) via `llmsc-core`. shell (M2 follow-up) and cp (M6)
//! are still stubs.

use clap::{Parser, Subcommand};
use llmsc_core::config::{Config, Sandbox};
use llmsc_core::display::{display_plan, DisplayCtx};
use llmsc_core::incus::{CliIncus, IncusClient};
use llmsc_core::process::SystemRunner;
use llmsc_core::progress::Reporter;
use llmsc_core::reconcile::reconcile;

/// Prints each step to stderr so progress shows during long operations (e.g. image pulls).
struct ConsoleReporter;

impl Reporter for ConsoleReporter {
    fn step(&self, msg: &str) {
        eprintln!("→ {msg}");
    }
}

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
    Launch {
        name: String,
        /// Base image (Incus image ref). Alpine by default — minimal + fast.
        #[arg(long, default_value = "images:alpine/3.21")]
        image: String,
    },
    /// List sandboxes.
    Ls,
    /// Open a shell as `user@sandbox`.
    Shell { target: String },
    /// Copy files (host↔container, container↔container).
    Cp { src: String, dst: String },
    /// Remove a sandbox.
    Rm { name: String },
    /// Reconcile declared sandboxes (from llmsc.toml) into Incus.
    Apply,
    /// Show or enforce a sandbox's network egress policy (the per-container ACL ring).
    Egress {
        /// Sandbox name.
        name: String,
        /// Compile + apply the policy (default just shows the compiled ACL).
        #[arg(long)]
        apply: bool,
    },
    /// Show how to view a sandbox's GUI on the host (the display recipe).
    Display {
        /// Sandbox name.
        name: String,
        /// Apply the display transport (add/remove the xpra Incus proxy device).
        #[arg(long)]
        apply: bool,
    },
    /// Control an agent: pause / resume / stop / steer (target is `agent@sandbox`).
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Mount the shared SeaweedFS-backed volume into a sandbox.
    MountShared {
        /// Sandbox name.
        name: String,
        /// Mount path inside the sandbox.
        #[arg(default_value = "/shared")]
        path: String,
    },
}

#[derive(Subcommand)]
enum AgentAction {
    /// Pause the agent (SIGSTOP all its processes).
    Pause { target: String },
    /// Resume the agent (SIGCONT).
    Resume { target: String },
    /// Stop the agent (SIGTERM).
    Stop { target: String },
    /// Inject a steering message into the agent's mailbox.
    Steer { target: String, message: String },
}

fn vm_name() -> String {
    // M2 follow-up will load this from the on-disk config.
    Config::default().vm.name
}

fn run() -> Result<(), String> {
    let runner = SystemRunner;
    let vm = vm_name();
    let incus = CliIncus::new(vm, &runner);

    match Cli::parse().command {
        Command::Launch { name, image } => {
            let spec = Sandbox {
                name: name.clone(),
                image,
                nesting: false,
                users: Vec::new(),
                ..Default::default()
            };
            incus
                .launch(&spec, &ConsoleReporter)
                .map_err(|e| e.to_string())?;
            println!("sandbox '{name}' launched");
        }
        Command::Ls => {
            let items = incus.sandboxes().map_err(|e| e.to_string())?;
            if items.is_empty() {
                println!("(no sandboxes)");
            } else {
                for i in items {
                    println!("{:<24} {:?}", i.name, i.status);
                }
            }
        }
        Command::Rm { name } => {
            incus.delete(&name).map_err(|e| e.to_string())?;
            println!("sandbox '{name}' removed");
        }
        Command::Shell { target } => {
            let (user, sandbox) = target
                .split_once('@')
                .ok_or_else(|| format!("target must be user@sandbox (got '{target}')"))?;
            let code = incus.shell(user, sandbox).map_err(|e| e.to_string())?;
            std::process::exit(code);
        }
        Command::Apply => {
            let cfg = Config::load_effective().map_err(|e| e.to_string())?;
            let report = reconcile(&cfg, &incus, &ConsoleReporter).map_err(|e| e.to_string())?;
            println!("created:  {:?}", report.created);
            println!("existing: {:?}", report.existing);
            if !report.extra.is_empty() {
                println!("drift (in Incus, not in config): {:?}", report.extra);
            }
        }
        Command::Egress { name, apply } => {
            let cfg = Config::load_effective().map_err(|e| e.to_string())?;
            let sb = cfg
                .sandbox(&name)
                .ok_or_else(|| format!("'{name}' is not config-managed"))?;
            if apply {
                let n = incus
                    .reconcile_egress(sb, &ConsoleReporter)
                    .map_err(|e| e.to_string())?;
                println!(
                    "{}",
                    if n == 0 {
                        "egress torn down (open/unmanaged)".to_string()
                    } else {
                        format!("egress enforced — {n} ACL change(s)")
                    }
                );
            } else {
                let ctx = incus.enforce_ctx(&name);
                match llmsc_core::enforce::egress_acl(sb, &ctx) {
                    Some(acl) => {
                        println!("ACL {} (egress):", acl.name);
                        if acl.egress.is_empty() {
                            println!("  (no allow rules — default-drop blocks all egress)");
                        }
                        for r in &acl.egress {
                            println!(
                                "  {} {} {}/{}  {}",
                                r.action, r.destination, r.port, r.protocol, r.description
                            );
                        }
                        println!("(use --apply to enforce)");
                    }
                    None => println!("egress is open/unmanaged — no ACL"),
                }
            }
        }
        Command::Display { name, apply } => {
            let cfg = Config::load_effective().map_err(|e| e.to_string())?;
            let sb = cfg
                .sandbox(&name)
                .ok_or_else(|| format!("'{name}' is not config-managed"))?;
            if apply {
                let n = incus
                    .reconcile_display(sb, &ConsoleReporter)
                    .map_err(|e| e.to_string())?;
                println!(
                    "{}",
                    match (sb.display.id(), n) {
                        ("xpra", _) =>
                            "display transport applied (xpra proxy device bound)".to_string(),
                        (_, 0) => "no display transport (none/x11 — nothing to bind)".to_string(),
                        (_, _) => "display transport torn down".to_string(),
                    }
                );
                return Ok(());
            }
            let ctx = DisplayCtx {
                vm_ssh: format!("lima-{}", cfg.vm.name),
                ..Default::default()
            };
            match display_plan(sb, &ctx) {
                None => {
                    println!("display: none — no remote display configured for '{name}'");
                    println!(
                        "  set `display = \"xpra\"` or `\"x11\"` under [[sandbox]] in llmsc.toml"
                    );
                }
                Some(plan) => {
                    println!(
                        "display: {} — view {name}'s GUI on the host:",
                        plan.method.id()
                    );
                    for (i, step) in plan.steps.iter().enumerate() {
                        println!("  {}. {}", i + 1, step.note);
                        println!("     $ {}", step.cmd);
                    }
                }
            }
        }
        Command::Agent { action } => {
            let split = |t: &str| -> Result<(String, String), String> {
                t.split_once('@')
                    .map(|(a, s)| (a.to_string(), s.to_string()))
                    .ok_or_else(|| format!("target must be agent@sandbox (got '{t}')"))
            };
            match action {
                AgentAction::Pause { target } => {
                    let (a, s) = split(&target)?;
                    incus
                        .signal_user(&s, &a, "STOP")
                        .map_err(|e| e.to_string())?;
                    println!("paused {a}@{s}");
                }
                AgentAction::Resume { target } => {
                    let (a, s) = split(&target)?;
                    incus
                        .signal_user(&s, &a, "CONT")
                        .map_err(|e| e.to_string())?;
                    println!("resumed {a}@{s}");
                }
                AgentAction::Stop { target } => {
                    let (a, s) = split(&target)?;
                    incus
                        .signal_user(&s, &a, "TERM")
                        .map_err(|e| e.to_string())?;
                    println!("stopped {a}@{s}");
                }
                AgentAction::Steer { target, message } => {
                    let (a, s) = split(&target)?;
                    incus
                        .steer_user(&s, &a, &message)
                        .map_err(|e| e.to_string())?;
                    println!("steered {a}@{s}");
                }
            }
        }
        Command::MountShared { name, path } => {
            incus
                .attach_shared_volume(&name, &path)
                .map_err(|e| e.to_string())?;
            println!("mounted shared volume into {name} at {path}");
        }
        Command::Cp { src, dst } => println!("cp {src} -> {dst}: not yet implemented (M6)"),
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
