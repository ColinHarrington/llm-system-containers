//! Tauri command layer for the llmsc GUI.
//!
//! These commands are thin wrappers over `llmsc-core` (the same logic the CLIs use) — they
//! shell out to Lima/Incus on the host. Long operations report progress with a silent reporter
//! for now (streaming progress to the GUI is a follow-up).

use llmsc_core::bootstrap::IncusBootstrap;
use llmsc_core::config::{self, Config, Sandbox, User, UserRole};
use llmsc_core::deploy::LiteLlmDeployer;
use llmsc_core::incus::{CliIncus, IncusClient, InstanceStatus};
use llmsc_core::process::SystemRunner;
use llmsc_core::progress::Reporter;
use llmsc_core::service;
use llmsc_core::vm::{LimaVmDriver, VmDriver, VmStatus};

/// Bytes → GB rounded to one decimal (display units for the resource meters).
fn to_gb(bytes: u64) -> f64 {
    ((bytes as f64 / 1024.0 / 1024.0 / 1024.0) * 10.0).round() / 10.0
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HostResourcesDto {
    cpu_used: f64,
    cpu_total: f64,
    mem_used: f64,
    mem_total: f64,
    disk_used: f64,
    disk_total: f64,
}

/// Live host/VM resource usage for the Dashboard meters (CPU cores, memory & disk in GB).
#[tauri::command]
fn host_resources() -> Result<HostResourcesDto, String> {
    let r = LimaVmDriver::new(Config::default().vm, SystemRunner)
        .resources()
        .map_err(|e| e.to_string())?;
    Ok(HostResourcesDto {
        cpu_used: (r.cpu_used * 10.0).round() / 10.0,
        cpu_total: r.cpu_total as f64,
        mem_used: to_gb(r.mem_used_bytes),
        mem_total: to_gb(r.mem_total_bytes),
        disk_used: to_gb(r.disk_used_bytes),
        disk_total: to_gb(r.disk_total_bytes),
    })
}
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Payload for the `progress` event the GUI listens on.
#[derive(Clone, Serialize)]
struct ProgressPayload {
    msg: String,
}

/// A [`Reporter`] that forwards each step to the frontend as a `progress` Tauri event.
struct EventReporter {
    app: AppHandle,
}

impl Reporter for EventReporter {
    fn step(&self, msg: &str) {
        let _ = self.app.emit("progress", ProgressPayload { msg: msg.to_string() });
    }
}

fn vm_name() -> String {
    Config::default().vm.name
}

fn vm_driver() -> LimaVmDriver<SystemRunner> {
    LimaVmDriver::new(Config::default().vm, SystemRunner)
}

fn status_str(s: VmStatus) -> String {
    match s {
        VmStatus::NotCreated => "NotCreated",
        VmStatus::Stopped => "Stopped",
        VmStatus::Starting => "Starting",
        VmStatus::Running => "Running",
    }
    .to_string()
}

#[tauri::command]
fn vm_status() -> Result<String, String> {
    vm_driver().status().map(status_str).map_err(|e| e.to_string())
}

#[tauri::command]
fn vm_up(app: AppHandle) -> Result<(), String> {
    let reporter = EventReporter { app };
    let cfg = Config::default();
    let name = cfg.vm.name.clone();
    LimaVmDriver::new(cfg.vm, SystemRunner)
        .up(&reporter)
        .map_err(|e| e.to_string())?;
    IncusBootstrap::new(name, &SystemRunner)
        .run(&reporter)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn vm_down() -> Result<(), String> {
    vm_driver().down().map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct SandboxDto {
    name: String,
    status: String,
    image: Option<String>,
}

#[tauri::command]
fn sandbox_list() -> Result<Vec<SandboxDto>, String> {
    let runner = SystemRunner;
    let incus = CliIncus::new(vm_name(), &runner);
    // Sandboxes only — service containers (svc-*) are infrastructure, never sandboxes.
    let items = incus.sandboxes().map_err(|e| e.to_string())?;
    Ok(items
        .into_iter()
        .map(|i| SandboxDto {
            name: i.name,
            status: match i.status {
                InstanceStatus::Running => "Running",
                InstanceStatus::Stopped => "Stopped",
            }
            .to_string(),
            image: None,
        })
        .collect())
}

#[tauri::command]
fn sandbox_launch(
    app: AppHandle,
    name: String,
    image: String,
    nesting: bool,
    operator: String,
    description: String,
    ephemeral: bool,
) -> Result<(), String> {
    let reporter = EventReporter { app };
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let desc = if description.trim().is_empty() { None } else { Some(description.trim().to_string()) };
    // Every sandbox gets exactly one human user (the operator) by default; agents are added later.
    let spec = Sandbox {
        name: name.clone(),
        image: image.clone(),
        nesting,
        ephemeral,
        description: desc.clone(),
        users: vec![User { name: operator.clone(), role: UserRole::Human, profile: None }],
        ..Default::default()
    };
    incus.launch(&spec, &reporter).map_err(|e| e.to_string())?;
    // Persist the full intent into config (best-effort; the sandbox is already created).
    let mut cfg = load_user_config().unwrap_or_default();
    {
        let sb = cfg.upsert_sandbox(&name, &image, nesting);
        sb.ephemeral = ephemeral;
        sb.description = desc;
    }
    cfg.set_sandbox_user(&name, User { name: operator, role: UserRole::Human, profile: None });
    let _ = cfg.save(&config::user_config_path());
    Ok(())
}

/// The default operator (human) username — from config, falling back to the host username.
#[tauri::command]
fn operator_default() -> String {
    Config::load_effective()
        .map(|c| c.operator)
        .unwrap_or_else(|_| config::default_operator_username())
}

/// Add an agent (one Linux user) to a running sandbox, with an optional profile.
#[tauri::command]
fn add_agent(app: AppHandle, sandbox: String, name: String, profile: String) -> Result<(), String> {
    let reporter = EventReporter { app };
    let suffix = if profile.is_empty() { String::new() } else { format!(" ({profile})") };
    reporter.step(&format!("Adding agent '{name}' to {sandbox}{suffix}"));
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    incus.add_user(&sandbox, &name, false).map_err(|e| e.to_string())?;
    // Persist the agent + its profile into config when the sandbox is config-managed.
    let mut cfg = load_user_config().unwrap_or_default();
    let user = User {
        name: name.clone(),
        role: UserRole::Agent,
        profile: if profile.is_empty() { None } else { Some(profile) },
    };
    if cfg.set_sandbox_user(&sandbox, user) {
        let _ = cfg.save(&config::user_config_path());
    }
    reporter.step(&format!("Agent '{name}' added{suffix}"));
    Ok(())
}

/// Remove an agent (its Linux user) from a sandbox, and drop it from config.
#[tauri::command]
fn remove_agent(app: AppHandle, sandbox: String, name: String) -> Result<(), String> {
    let reporter = EventReporter { app };
    reporter.step(&format!("Removing agent '{name}' from {sandbox}"));
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    incus.remove_user(&sandbox, &name).map_err(|e| e.to_string())?;
    let mut cfg = load_user_config().unwrap_or_default();
    if cfg.remove_sandbox_user(&sandbox, &name) {
        let _ = cfg.save(&config::user_config_path());
    }
    reporter.step(&format!("Agent '{name}' removed"));
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileDto {
    name: String,
    summary: String,
    filesystem: String,
    network: String,
    l3: bool,
    llm_budget: String,
    control_plane: String,
}

/// The shipped agent-profile archetypes (the definition layer; enforcement is later).
#[tauri::command]
fn profiles() -> Vec<ProfileDto> {
    llmsc_core::profile::catalog()
        .iter()
        .map(|p| ProfileDto {
            name: p.name.to_string(),
            summary: p.summary.to_string(),
            filesystem: p.filesystem.to_string(),
            network: p.network.to_string(),
            l3: p.l3,
            llm_budget: p.llm_budget.to_string(),
            control_plane: p.control_plane.to_string(),
        })
        .collect()
}

#[tauri::command]
fn sandbox_rm(name: String) -> Result<(), String> {
    let runner = SystemRunner;
    let incus = CliIncus::new(vm_name(), &runner);
    incus.delete(&name).map_err(|e| e.to_string())?;
    let mut cfg = load_user_config().unwrap_or_default();
    if cfg.remove_sandbox(&name) {
        let _ = cfg.save(&config::user_config_path());
    }
    Ok(())
}

#[derive(Serialize)]
struct TopoAgentDto {
    name: String,
    kind: String,
    state: String,
    action: String,
    tools: Vec<String>,
    active: Option<String>,
    profile: Option<String>,
}

#[derive(Serialize)]
struct TopoSandboxDto {
    name: String,
    image: String,
    status: String,
    l3: bool,
    cpu: String,
    mem: String,
    agents: Vec<TopoAgentDto>,
}

fn fmt_mem(bytes: u64) -> String {
    if bytes == 0 {
        return "—".to_string();
    }
    let mb = bytes as f64 / 1024.0 / 1024.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.0} MB")
    }
}

/// Real topology: sandboxes (services excluded) with their Incus status/image/nesting/memory and
/// their Linux users (one per agent + the human operator). Live per-agent activity (tool use,
/// task) is not instrumented yet, so users report as idle with no activity — honest, not faked.
#[tauri::command]
fn topology() -> Result<Vec<TopoSandboxDto>, String> {
    let runner = SystemRunner;
    let incus = CliIncus::new(vm_name(), &runner);
    // Config is authoritative for role + assigned profile; live Incus is authoritative for which
    // users actually exist. Merge: live users, enriched with config role/profile where recorded.
    let cfg = Config::load_effective().unwrap_or_default();
    let sandboxes = incus.topology().map_err(|e| e.to_string())?;
    Ok(sandboxes
        .into_iter()
        .map(|s| {
            let running = s.status == InstanceStatus::Running;
            let cfg_sb = cfg.sandbox(&s.name);
            TopoSandboxDto {
                status: if running { "running" } else { "stopped" }.to_string(),
                l3: s.nesting,
                cpu: "—".to_string(),
                mem: if running { fmt_mem(s.mem_bytes) } else { "—".to_string() },
                image: s.image,
                agents: s
                    .users
                    .into_iter()
                    .map(|u| {
                        let cu = cfg_sb.and_then(|sb| sb.users.iter().find(|x| x.name == u.name));
                        let human = match cu {
                            Some(c) => matches!(c.role, UserRole::Human),
                            None => u.human || u.name == cfg.operator,
                        };
                        TopoAgentDto {
                            kind: if human { "human" } else { "agent" }.to_string(),
                            profile: cu.and_then(|c| c.profile.clone()),
                            name: u.name,
                            state: "idle".to_string(),
                            action: String::new(),
                            tools: Vec::new(),
                            active: None,
                        }
                    })
                    .collect(),
                name: s.name,
            }
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkDto {
    name: String,
    kind: String,
    ipv4: String,
    nat: bool,
    used_by: usize,
}

#[derive(Serialize)]
struct SandboxNetDto {
    name: String,
    status: String,
    networks: Vec<String>,
    ipv4: String,
}

#[derive(Serialize)]
struct NetworkingDto {
    networks: Vec<NetworkDto>,
    sandboxes: Vec<SandboxNetDto>,
}

/// Real networking: the VM's managed Incus networks and which sandboxes attach to which (with
/// addresses). Egress policy / inspection / Tetragon enforcement are not implemented yet (M4),
/// so this reports topology only — it does not claim policy that nothing enforces.
#[tauri::command]
fn networking() -> Result<NetworkingDto, String> {
    let runner = SystemRunner;
    let incus = CliIncus::new(vm_name(), &runner);
    let networks = incus.networks().map_err(|e| e.to_string())?;
    let sandboxes = incus.sandbox_networks().map_err(|e| e.to_string())?;
    Ok(NetworkingDto {
        networks: networks
            .into_iter()
            .map(|n| NetworkDto { name: n.name, kind: n.kind, ipv4: n.ipv4, nat: n.nat, used_by: n.used_by })
            .collect(),
        sandboxes: sandboxes
            .into_iter()
            .map(|s| SandboxNetDto {
                status: if s.status == InstanceStatus::Running { "running" } else { "stopped" }.to_string(),
                networks: s.networks,
                ipv4: s.ipv4,
                name: s.name,
            })
            .collect(),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageDto {
    name: String,
    desc: String,
    flavor: String,
    base: String,
    arch: String,
    size: String,
    used_by: String,
    updated: String,
}

fn to_image_dto(i: llmsc_core::incus::ImageRecord) -> ImageDto {
    ImageDto {
        name: i.name,
        desc: i.description,
        flavor: i.flavor,
        base: i.base,
        arch: i.arch,
        size: fmt_mem(i.size_bytes),
        used_by: match i.used_by {
            0 => "—".to_string(),
            1 => "1 sandbox".to_string(),
            n => format!("{n} sandboxes"),
        },
        updated: if i.uploaded.is_empty() { "—".to_string() } else { i.uploaded },
    }
}

/// Build a custom image from a base + packages/script, streaming progress to the GUI.
/// Packages are installed cross-distro (apt, falling back to apk) ahead of the user's script.
#[tauri::command]
fn build_image(
    app: AppHandle,
    base: String,
    name: String,
    packages: Vec<String>,
    script: String,
    description: String,
) -> Result<(), String> {
    let reporter = EventReporter { app };
    let mut setup = String::new();
    let pkgs: Vec<&str> = packages.iter().map(|s| s.as_str()).filter(|s| !s.is_empty()).collect();
    if !pkgs.is_empty() {
        let list = pkgs.join(" ");
        setup.push_str(&format!(
            "(apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y {list}) || apk add --no-cache {list}\n"
        ));
    }
    setup.push_str(&script);
    CliIncus::new(vm_name(), &SystemRunner)
        .build_image(&base, &name, &setup, &description, &reporter)
        .map_err(|e| e.to_string())
}

/// Container images only — sandboxes are system containers, so virtual-machine images are
/// never launchable here and are excluded.
fn container_images(imgs: Vec<llmsc_core::incus::ImageRecord>) -> Vec<ImageDto> {
    imgs.into_iter()
        .filter(|i| i.kind != "virtual-machine")
        .map(to_image_dto)
        .collect()
}

/// Images cached locally in the VM (base distros pulled on first use + custom builds).
#[tauri::command]
fn images() -> Result<Vec<ImageDto>, String> {
    let runner = SystemRunner;
    let incus = CliIncus::new(vm_name(), &runner);
    Ok(container_images(incus.images().map_err(|e| e.to_string())?))
}

/// All container images available from the `images:` remote catalog. Hits the network and can be
/// large — the GUI fetches this on demand when the user switches to the "All available" filter.
#[tauri::command]
fn images_available() -> Result<Vec<ImageDto>, String> {
    let runner = SystemRunner;
    let incus = CliIncus::new(vm_name(), &runner);
    Ok(container_images(incus.images_remote("images").map_err(|e| e.to_string())?))
}

#[derive(Serialize)]
struct ServiceDto {
    name: String,
    description: String,
    priority: String,
    enabled: bool,
}

#[tauri::command]
fn service_list() -> Result<Vec<ServiceDto>, String> {
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    Ok(service::catalog()
        .iter()
        .map(|e| ServiceDto {
            name: e.name.to_string(),
            description: e.description.to_string(),
            priority: e.priority.to_string(),
            enabled: cfg.service_enabled(e.name),
        })
        .collect())
}

fn load_user_config() -> Result<Config, String> {
    let path = config::user_config_path();
    if path.exists() {
        Config::load(&path).map_err(|e| e.to_string())
    } else {
        Ok(Config::default())
    }
}

#[tauri::command]
fn service_enable(name: String) -> Result<(), String> {
    let entry = service::lookup(&name).ok_or_else(|| format!("unknown service '{name}'"))?;
    let mut cfg = load_user_config()?;
    cfg.enable_service(&name, entry.default_placement);
    cfg.save(&config::user_config_path()).map_err(|e| e.to_string())
}

#[tauri::command]
fn service_disable(name: String) -> Result<(), String> {
    let mut cfg = load_user_config()?;
    cfg.disable_service(&name);
    cfg.save(&config::user_config_path()).map_err(|e| e.to_string())
}

/// Provision (stand up) a single enabled service in the VM. Only services with a deployer are
/// supported; others return an error. Progress streams to the GUI via the `progress` event.
#[tauri::command]
fn service_up(app: AppHandle, name: String) -> Result<(), String> {
    let reporter = EventReporter { app };
    let vm = vm_name();
    match name.as_str() {
        "litellm" => LiteLlmDeployer::new(vm, &SystemRunner)
            .deploy(&reporter)
            .map_err(|e| e.to_string()),
        other => Err(format!("no deployer yet for '{other}'")),
    }
}

#[derive(Deserialize)]
struct SetupCfg {
    #[serde(default)]
    operator: String,
    cpus: u32,
    #[serde(rename = "memoryGib")]
    memory_gib: u32,
    #[serde(rename = "diskGib")]
    disk_gib: u32,
    services: Vec<String>,
    #[serde(rename = "defaultDenyEgress")]
    default_deny_egress: bool,
}

#[tauri::command]
fn platform_init(app: AppHandle, cfg: SetupCfg) -> Result<(), String> {
    let reporter = EventReporter { app };
    let _ = cfg.default_deny_egress; // networking policy is M4 (deferred); accepted for now.
    let mut c = Config::default();
    if !cfg.operator.trim().is_empty() {
        c.operator = cfg.operator.trim().to_string();
    }
    c.vm.cpus = cfg.cpus;
    c.vm.memory_gib = cfg.memory_gib;
    c.vm.disk_gib = cfg.disk_gib;
    for s in &cfg.services {
        if let Some(entry) = service::lookup(s) {
            c.enable_service(s, entry.default_placement);
        }
    }
    c.save(&config::user_config_path()).map_err(|e| e.to_string())?;
    reporter.step("Wrote configuration");
    let name = c.vm.name.clone();
    LimaVmDriver::new(c.vm, SystemRunner)
        .up(&reporter)
        .map_err(|e| e.to_string())?;
    IncusBootstrap::new(name, &SystemRunner)
        .run(&reporter)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            vm_status,
            vm_up,
            vm_down,
            sandbox_list,
            sandbox_launch,
            sandbox_rm,
            operator_default,
            add_agent,
            remove_agent,
            profiles,
            topology,
            host_resources,
            images,
            images_available,
            build_image,
            networking,
            service_list,
            service_enable,
            service_disable,
            service_up,
            platform_init
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
