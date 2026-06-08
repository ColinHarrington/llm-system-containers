//! `llmsc` — manage individual LLM System Containers (L2 sandboxes).
//!
//! launch/ls/rm/cp drive Incus via `llmsc-core` (the `vm` or `local` deployment target).

use clap::{Parser, Subcommand};
use llmsc_core::config::{effective_config_path, Config, DeploymentMode, DisplayMethod, Sandbox};
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
        /// Remote display method for GUI apps: none (default) | xpra | x11.
        #[arg(long, default_value = "none")]
        display: String,
    },
    /// List sandboxes.
    Ls,
    /// Show a sandbox's configured intent (target, image, display, egress, users).
    Info { name: String },
    /// Open a shell as `user@sandbox`.
    Shell { target: String },
    /// Run a command in a sandbox: `llmsc exec [user@]sandbox -- <cmd>`.
    Exec {
        /// `user@sandbox` (runs via that user) or `sandbox` (runs as root).
        target: String,
        /// The command to run (everything after `--`).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        cmd: Vec<String>,
    },
    /// Copy a file host↔container. One side is a container ref `name:/abs/path`, the other a
    /// host path, e.g. `llmsc cp ./f web:/work/f` or `llmsc cp web:/work/f ./f`.
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
    /// Toggle a sandbox's security hardening (persisted to the effective config).
    Harden {
        /// Sandbox name.
        name: String,
        /// NIC anti-spoof filtering: `on` | `off`.
        #[arg(long)]
        nic_filtering: Option<String>,
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

fn run() -> Result<(), String> {
    let runner = SystemRunner;
    // Parse first so --help/--version work without touching config; then resolve the deployment
    // target from the effective config (honors a configured VM name; errors on local/remote).
    let command = Cli::parse().command;
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    // Build the Incus client for the configured deployment target.
    let incus = match cfg.mode {
        DeploymentMode::Vm => CliIncus::new(cfg.vm.name.clone(), &runner),
        DeploymentMode::Local => CliIncus::local(&runner),
        DeploymentMode::Remote => {
            return Err(
                "deployment target 'remote' is not supported yet (use 'vm' or 'local')".into(),
            )
        }
    };

    match command {
        Command::Launch {
            name,
            image,
            display,
        } => {
            let method = DisplayMethod::from_id(&display)
                .ok_or_else(|| format!("unknown display method '{display}' (none|xpra|x11)"))?;
            let spec = Sandbox {
                name: name.clone(),
                image,
                nesting: false,
                users: Vec::new(),
                display: method,
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
        Command::Info { name } => {
            let sb = cfg
                .sandbox(&name)
                .ok_or_else(|| format!("'{name}' is not config-managed"))?;
            println!("sandbox: {}", sb.name);
            println!("  image:         {}", sb.image);
            println!("  target:        {}", cfg.mode.id());
            println!("  display:       {}", sb.display.id());
            println!("  nesting (L3):  {}", sb.nesting);
            println!("  net-filtering: {}", sb.net_filtering);
            println!(
                "  egress:        {}",
                sb.egress
                    .as_ref()
                    .map(|e| e.posture.id())
                    .unwrap_or("unmanaged")
            );
            if !sb.users.is_empty() {
                let users: Vec<&str> = sb.users.iter().map(|u| u.name.as_str()).collect();
                println!("  users:         {}", users.join(", "));
            }
        }
        Command::Rm { name } => {
            incus.delete(&name).map_err(|e| e.to_string())?;
            println!("sandbox '{name}' removed");
        }
        Command::Exec { target, cmd } => {
            if cmd.is_empty() {
                return Err("no command given (usage: llmsc exec [user@]sandbox -- <cmd>)".into());
            }
            let (user, name) = match target.split_once('@') {
                Some((u, n)) => (Some(u), n),
                None => (None, target.as_str()),
            };
            let argv: Vec<&str> = cmd.iter().map(String::as_str).collect();
            let code = incus.exec(name, user, &argv).map_err(|e| e.to_string())?;
            std::process::exit(code);
        }
        Command::Shell { target } => {
            let (user, sandbox) = target
                .split_once('@')
                .ok_or_else(|| format!("target must be user@sandbox (got '{target}')"))?;
            let code = incus.shell(user, sandbox).map_err(|e| e.to_string())?;
            std::process::exit(code);
        }
        Command::Apply => {
            let report = reconcile(&cfg, &incus, &ConsoleReporter).map_err(|e| e.to_string())?;
            println!("created:  {:?}", report.created);
            println!("existing: {:?}", report.existing);
            if !report.extra.is_empty() {
                println!("drift (in Incus, not in config): {:?}", report.extra);
            }
        }
        Command::Egress { name, apply } => {
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
        Command::Harden {
            name,
            nic_filtering,
        } => {
            let on = match nic_filtering.as_deref() {
                Some("on") => true,
                Some("off") => false,
                Some(other) => {
                    return Err(format!("--nic-filtering must be on|off (got '{other}')"))
                }
                None => return Err("nothing to harden (use --nic-filtering on|off)".into()),
            };
            let mut c = Config::load_effective().map_err(|e| e.to_string())?;
            if !c.set_sandbox_net_filtering(&name, on) {
                return Err(format!("'{name}' is not config-managed"));
            }
            c.save(&effective_config_path())
                .map_err(|e| e.to_string())?;
            println!("{name}: nic-filtering {}", if on { "on" } else { "off" });
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
        Command::Cp { src, dst } => {
            use CpRef::{Container, Host};
            match (parse_cp_ref(&src), parse_cp_ref(&dst)) {
                (Host(h), Container { name, path }) => {
                    incus
                        .push_file(&h, &name, &path)
                        .map_err(|e| e.to_string())?;
                    println!("copied {h} -> {name}:{path}");
                }
                (Container { name, path }, Host(h)) => {
                    incus
                        .pull_file(&name, &path, &h)
                        .map_err(|e| e.to_string())?;
                    println!("copied {name}:{path} -> {h}");
                }
                (Container { .. }, Container { .. }) => {
                    return Err("container↔container copy is not supported yet".into());
                }
                (Host(_), Host(_)) => {
                    return Err(
                        "both paths are host paths — one side must be a container ref (name:/path)"
                            .into(),
                    );
                }
            }
        }
    }
    Ok(())
}

/// A `cp` endpoint: a host path, or a container ref `name:/abs/path`.
enum CpRef {
    Host(String),
    Container { name: String, path: String },
}

/// Parse a `cp` argument. `name:/path` (name without `/`, path absolute) → a container ref;
/// anything else (incl. relative `name:path` or a path with no `:`) → a host path.
fn parse_cp_ref(s: &str) -> CpRef {
    if let Some((name, path)) = s.split_once(':') {
        if !name.is_empty() && !name.contains('/') && path.starts_with('/') {
            return CpRef::Container {
                name: name.to_string(),
                path: path.to_string(),
            };
        }
    }
    CpRef::Host(s.to_string())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_cp_ref, CpRef};

    #[test]
    fn parse_cp_ref_distinguishes_container_and_host() {
        match parse_cp_ref("web:/work/f") {
            CpRef::Container { name, path } => {
                assert_eq!(name, "web");
                assert_eq!(path, "/work/f");
            }
            _ => panic!("expected container ref"),
        }
        // No colon, relative-after-colon, and slash-in-name are all host paths.
        assert!(matches!(parse_cp_ref("./f"), CpRef::Host(_)));
        assert!(matches!(parse_cp_ref("web:rel/path"), CpRef::Host(_)));
        assert!(matches!(parse_cp_ref("a/b:/x"), CpRef::Host(_)));
    }
}
