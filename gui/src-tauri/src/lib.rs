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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HostResourcesDto {
    cpu_used: f64,
    cpu_total: f64,
    /// Memory and disk are raw **bytes** — the GUI formats them (MB / GB) for granularity.
    mem_used: f64,
    mem_total: f64,
    disk_used: f64,
    disk_total: f64,
}

/// Live host/VM resource usage for the Dashboard meters (CPU in cores; memory & disk in bytes).
#[tauri::command]
fn host_resources() -> Result<HostResourcesDto, String> {
    let r = LimaVmDriver::new(Config::default().vm, SystemRunner)
        .resources()
        .map_err(|e| e.to_string())?;
    Ok(HostResourcesDto {
        cpu_used: (r.cpu_used * 10.0).round() / 10.0,
        cpu_total: r.cpu_total as f64,
        mem_used: r.mem_used_bytes as f64,
        mem_total: r.mem_total_bytes as f64,
        disk_used: r.disk_used_bytes as f64,
        disk_total: r.disk_total_bytes as f64,
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
        let _ = self.app.emit(
            "progress",
            ProgressPayload {
                msg: msg.to_string(),
            },
        );
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
    vm_driver()
        .status()
        .map(status_str)
        .map_err(|e| e.to_string())
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MountSpec {
    source: String,
    path: String,
    #[serde(default)]
    readonly: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewSandboxSpec {
    name: String,
    image: String,
    operator: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    ephemeral: bool,
    #[serde(default)]
    nesting: bool,
    #[serde(default)]
    profiles: Vec<String>,
    #[serde(default)]
    mounts: Vec<MountSpec>,
    #[serde(default)]
    cloud_init: String,
    #[serde(default)]
    network: String,
    #[serde(default)]
    cpu_limit: String,
    #[serde(default)]
    memory_limit: String,
}

/// Sanitize a mount path into an Incus device name (`/work/src` → `work-src`).
fn device_name(path: &str, i: usize) -> String {
    let s: String = path
        .trim_matches('/')
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-');
    if s.is_empty() {
        format!("mount{i}")
    } else {
        s.to_string()
    }
}

#[tauri::command]
fn sandbox_launch(app: AppHandle, spec: NewSandboxSpec) -> Result<(), String> {
    let reporter = EventReporter { app };
    let incus = CliIncus::new(vm_name(), &SystemRunner);

    let mut devices: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, String>,
    > = Default::default();
    for (i, m) in spec.mounts.iter().enumerate() {
        if m.source.trim().is_empty() || m.path.trim().is_empty() {
            continue;
        }
        let mut d = std::collections::BTreeMap::new();
        d.insert("type".into(), "disk".into());
        d.insert("source".into(), m.source.trim().to_string());
        d.insert("path".into(), m.path.trim().to_string());
        d.insert("shift".into(), "true".into()); // idmapped mount → usable in an unprivileged container
        if m.readonly {
            d.insert("readonly".into(), "true".into());
        }
        devices.insert(device_name(&m.path, i), d);
    }
    if !spec.network.trim().is_empty() {
        let mut nic = std::collections::BTreeMap::new();
        nic.insert("type".into(), "nic".into());
        nic.insert("network".into(), spec.network.trim().to_string());
        devices.insert("eth0".into(), nic);
    }

    let mut config: std::collections::BTreeMap<String, String> = Default::default();
    if !spec.cloud_init.trim().is_empty() {
        config.insert("cloud-init.user-data".into(), spec.cloud_init.clone());
    }
    if !spec.cpu_limit.trim().is_empty() {
        config.insert("limits.cpu".into(), spec.cpu_limit.trim().to_string());
    }
    if !spec.memory_limit.trim().is_empty() {
        config.insert("limits.memory".into(), spec.memory_limit.trim().to_string());
    }

    let desc = if spec.description.trim().is_empty() {
        None
    } else {
        Some(spec.description.trim().to_string())
    };
    let sandbox = Sandbox {
        name: spec.name.clone(),
        image: spec.image.clone(),
        description: desc,
        nesting: spec.nesting,
        ephemeral: spec.ephemeral,
        profiles: spec
            .profiles
            .iter()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect(),
        config,
        devices,
        // Every sandbox gets exactly one human user (the operator); agents are added later.
        users: vec![User {
            name: spec.operator.clone(),
            role: UserRole::Human,
            profile: None,
            guardrails: None,
        }],
        // Managed by default: agents may reach the LLM proxy and nothing else, until enforced.
        egress: Some(llmsc_core::config::EgressPolicy::default_managed()),
    };

    incus
        .launch(&sandbox, &reporter)
        .map_err(|e| e.to_string())?;
    // Persist the full intent into config (best-effort; the sandbox is already created).
    let mut cfg = load_user_config().unwrap_or_default();
    cfg.put_sandbox(sandbox);
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
fn add_agent(
    app: AppHandle,
    sandbox: String,
    name: String,
    profile: String,
    guardrails: Option<GuardrailsDto>,
) -> Result<(), String> {
    let reporter = EventReporter { app };
    let suffix = if profile.is_empty() {
        String::new()
    } else {
        format!(" ({profile})")
    };
    reporter.step(&format!("Adding agent '{name}' to {sandbox}{suffix}"));
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    incus
        .add_user(&sandbox, &name, false)
        .map_err(|e| e.to_string())?;
    // Persist the agent + its guardrails into config (sandbox must be config-managed).
    // The profile *seeds* the guardrails; explicit values from the create form override the seed.
    let mut cfg = load_user_config().unwrap_or_default();
    let guardrails = match guardrails {
        Some(g) => Some(llmsc_core::config::Guardrails {
            filesystem: g.filesystem,
            network: g.network,
            l3: g.l3,
            llm_budget: g.llm_budget,
            control_plane: g.control_plane,
        }),
        None if profile.is_empty() => None,
        None => llmsc_core::config::Guardrails::from_profile(&profile),
    };
    let user = User {
        name: name.clone(),
        role: UserRole::Agent,
        profile: if profile.is_empty() {
            None
        } else {
            Some(profile)
        },
        guardrails,
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
    incus
        .remove_user(&sandbox, &name)
        .map_err(|e| e.to_string())?;
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IncusProfileDto {
    name: String,
    description: String,
    used_by: usize,
    config: std::collections::BTreeMap<String, String>,
    devices: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
}

/// Recommended starter Incus profiles the user can apply into the project.
#[tauri::command]
fn starter_incus_profiles() -> Vec<IncusProfileDto> {
    llmsc_core::config::starter_incus_profiles()
        .into_iter()
        .map(|p| IncusProfileDto {
            name: p.name,
            description: p.description.unwrap_or_default(),
            used_by: 0,
            config: p.config,
            devices: p.devices,
        })
        .collect()
}

/// Apply (reconcile into the project) a starter or TOML-owned Incus profile, and record it in config.
#[tauri::command]
fn incus_profile_apply(app: AppHandle, name: String) -> Result<(), String> {
    let reporter = EventReporter { app };
    let cfg = load_user_config().unwrap_or_default();
    let desired = cfg
        .incus_profile(&name)
        .cloned()
        .or_else(|| {
            llmsc_core::config::starter_incus_profiles()
                .into_iter()
                .find(|p| p.name == name)
        })
        .ok_or_else(|| format!("unknown profile '{name}'"))?;
    CliIncus::new(vm_name(), &SystemRunner)
        .reconcile_profile(&desired, &reporter)
        .map_err(|e| e.to_string())?;
    let mut cfg2 = load_user_config().unwrap_or_default();
    cfg2.put_incus_profile(desired);
    save_user_config(&cfg2);
    Ok(())
}

/// Reconcile all TOML-owned Incus profiles into the project. Returns how many.
#[tauri::command]
fn reconcile_incus_profiles(app: AppHandle) -> Result<usize, String> {
    let reporter = EventReporter { app };
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    for p in &cfg.incus_profiles {
        incus
            .reconcile_profile(p, &reporter)
            .map_err(|e| e.to_string())?;
    }
    Ok(cfg.incus_profiles.len())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDto {
    name: String,
    description: String,
    used_by: usize,
    config: std::collections::BTreeMap<String, String>,
}

/// Incus projects (features / limits / restrictions).
#[tauri::command]
fn projects() -> Result<Vec<ProjectDto>, String> {
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let ps = incus.projects().map_err(|e| e.to_string())?;
    Ok(ps
        .into_iter()
        .map(|p| ProjectDto {
            name: p.name,
            description: p.description,
            used_by: p.used_by,
            config: p.config,
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageVolumeDto {
    name: String,
    vtype: String,
    used_by: usize,
    config: std::collections::BTreeMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoragePoolDto {
    name: String,
    driver: String,
    description: String,
    used_by: usize,
    config: std::collections::BTreeMap<String, String>,
    volumes: Vec<StorageVolumeDto>,
}

/// Storage pools (and their custom volumes) in the project.
#[tauri::command]
fn storage() -> Result<Vec<StoragePoolDto>, String> {
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let pools = incus.storage().map_err(|e| e.to_string())?;
    Ok(pools
        .into_iter()
        .map(|p| StoragePoolDto {
            name: p.name,
            driver: p.driver,
            description: p.description,
            used_by: p.used_by,
            config: p.config,
            volumes: p
                .volumes
                .into_iter()
                .map(|v| StorageVolumeDto {
                    name: v.name,
                    vtype: v.vtype,
                    used_by: v.used_by,
                    config: v.config,
                })
                .collect(),
        })
        .collect())
}

/// The Incus profiles (config+devices composition bundles) in the project.
#[tauri::command]
fn incus_profiles() -> Result<Vec<IncusProfileDto>, String> {
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let ps = incus.incus_profiles().map_err(|e| e.to_string())?;
    Ok(ps
        .into_iter()
        .map(|p| IncusProfileDto {
            name: p.name,
            description: p.description,
            used_by: p.used_by,
            config: p.config,
            devices: p.devices,
        })
        .collect())
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
#[serde(rename_all = "camelCase")]
struct InstanceConfigDto {
    name: String,
    status: String,
    description: String,
    ephemeral: bool,
    profiles: Vec<String>,
    config: std::collections::BTreeMap<String, String>,
    devices: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    local_devices: Vec<String>,
}

/// Read a sandbox's live Incus surface (config/devices/profiles) back from the server.
#[tauri::command]
fn instance_config(name: String) -> Result<InstanceConfigDto, String> {
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let i = incus.instance(&name).map_err(|e| e.to_string())?;
    Ok(InstanceConfigDto {
        name: i.name,
        status: if i.status == InstanceStatus::Running {
            "running"
        } else {
            "stopped"
        }
        .to_string(),
        description: i.description,
        ephemeral: i.ephemeral,
        profiles: i.profiles,
        config: i.config,
        devices: i.devices,
        local_devices: i.local_devices,
    })
}

fn save_user_config(cfg: &Config) {
    let _ = cfg.save(&config::user_config_path());
}

/// Render a sandbox's declared intent as the Incus instance YAML (`InstancePut`) — the artifact
/// `incus create <image> <name> < config.yaml` consumes.
#[tauri::command]
fn instance_yaml(name: String) -> Result<String, String> {
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    cfg.sandbox(&name)
        .map(|s| s.to_instance_yaml())
        .ok_or_else(|| format!("'{name}' is not config-managed"))
}

#[derive(Serialize)]
struct SnapshotDto {
    name: String,
    created: String,
    stateful: bool,
}

#[tauri::command]
fn snapshots(name: String) -> Result<Vec<SnapshotDto>, String> {
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    Ok(incus
        .snapshots(&name)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| SnapshotDto {
            name: s.name,
            created: s.created,
            stateful: s.stateful,
        })
        .collect())
}

#[tauri::command]
fn snapshot_create(app: AppHandle, name: String, snapshot: String) -> Result<(), String> {
    let reporter = EventReporter { app };
    reporter.step(&format!("Snapshotting {name} → {snapshot}"));
    CliIncus::new(vm_name(), &SystemRunner)
        .snapshot_create(&name, &snapshot)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn snapshot_restore(app: AppHandle, name: String, snapshot: String) -> Result<(), String> {
    let reporter = EventReporter { app };
    reporter.step(&format!("Restoring {name} to {snapshot}"));
    CliIncus::new(vm_name(), &SystemRunner)
        .snapshot_restore(&name, &snapshot)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn snapshot_delete(name: String, snapshot: String) -> Result<(), String> {
    CliIncus::new(vm_name(), &SystemRunner)
        .snapshot_delete(&name, &snapshot)
        .map_err(|e| e.to_string())
}

/// Converge a running instance toward its declared intent (config/devices/profiles). Returns the
/// number of changes applied (0 = already in sync). Streams each step to the GUI.
#[tauri::command]
fn apply_sandbox(app: AppHandle, name: String) -> Result<usize, String> {
    let reporter = EventReporter { app };
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let live = incus.instance(&name).map_err(|e| e.to_string())?;
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let desired = cfg
        .sandbox(&name)
        .ok_or_else(|| format!("'{name}' is not config-managed"))?;
    let plan = llmsc_core::reconcile::converge_plan(desired, &live);
    let n = plan.len();
    if n == 0 {
        reporter.step("already in sync");
        return Ok(0);
    }
    reporter.step(&format!("Converging {name} — {n} change(s)"));
    incus
        .converge(&name, &plan, &reporter)
        .map_err(|e| e.to_string())?;
    Ok(n)
}

/// Set a live instance config key and converge it into config intent.
#[tauri::command]
fn instance_set_config(name: String, key: String, value: String) -> Result<(), String> {
    CliIncus::new(vm_name(), &SystemRunner)
        .set_config(&name, &key, &value)
        .map_err(|e| e.to_string())?;
    let mut cfg = load_user_config().unwrap_or_default();
    if let Some(sb) = cfg.sandbox_mut(&name) {
        sb.config.insert(key, value);
        save_user_config(&cfg);
    }
    Ok(())
}

#[tauri::command]
fn instance_unset_config(name: String, key: String) -> Result<(), String> {
    CliIncus::new(vm_name(), &SystemRunner)
        .unset_config(&name, &key)
        .map_err(|e| e.to_string())?;
    let mut cfg = load_user_config().unwrap_or_default();
    if let Some(sb) = cfg.sandbox_mut(&name) {
        sb.config.remove(&key);
        save_user_config(&cfg);
    }
    Ok(())
}

#[tauri::command]
fn instance_add_mount(
    name: String,
    source: String,
    path: String,
    readonly: bool,
) -> Result<(), String> {
    let mut keys: std::collections::BTreeMap<String, String> = Default::default();
    keys.insert("type".into(), "disk".into());
    keys.insert("source".into(), source);
    keys.insert("path".into(), path.clone());
    keys.insert("shift".into(), "true".into());
    if readonly {
        keys.insert("readonly".into(), "true".into());
    }
    let dev = device_name(&path, 0);
    CliIncus::new(vm_name(), &SystemRunner)
        .add_device(&name, &dev, &keys)
        .map_err(|e| e.to_string())?;
    let mut cfg = load_user_config().unwrap_or_default();
    if let Some(sb) = cfg.sandbox_mut(&name) {
        sb.devices.insert(dev, keys);
        save_user_config(&cfg);
    }
    Ok(())
}

#[tauri::command]
fn instance_remove_device(name: String, device: String) -> Result<(), String> {
    CliIncus::new(vm_name(), &SystemRunner)
        .remove_device(&name, &device)
        .map_err(|e| e.to_string())?;
    let mut cfg = load_user_config().unwrap_or_default();
    if let Some(sb) = cfg.sandbox_mut(&name) {
        sb.devices.remove(&device);
        save_user_config(&cfg);
    }
    Ok(())
}

#[tauri::command]
fn instance_add_profile(name: String, profile: String) -> Result<(), String> {
    CliIncus::new(vm_name(), &SystemRunner)
        .add_profile(&name, &profile)
        .map_err(|e| e.to_string())?;
    let mut cfg = load_user_config().unwrap_or_default();
    if let Some(sb) = cfg.sandbox_mut(&name) {
        if !sb.profiles.contains(&profile) {
            sb.profiles.push(profile);
        }
        save_user_config(&cfg);
    }
    Ok(())
}

#[tauri::command]
fn instance_remove_profile(name: String, profile: String) -> Result<(), String> {
    CliIncus::new(vm_name(), &SystemRunner)
        .remove_profile(&name, &profile)
        .map_err(|e| e.to_string())?;
    let mut cfg = load_user_config().unwrap_or_default();
    if let Some(sb) = cfg.sandbox_mut(&name) {
        sb.profiles.retain(|p| p != &profile);
        save_user_config(&cfg);
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuardrailsDto {
    #[serde(default)]
    filesystem: String,
    #[serde(default)]
    network: String,
    #[serde(default)]
    l3: bool,
    #[serde(default)]
    llm_budget: String,
    #[serde(default)]
    control_plane: String,
}

fn to_guardrails_dto(g: llmsc_core::config::Guardrails) -> GuardrailsDto {
    GuardrailsDto {
        filesystem: g.filesystem,
        network: g.network,
        l3: g.l3,
        llm_budget: g.llm_budget,
        control_plane: g.control_plane,
    }
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
    guardrails: Option<GuardrailsDto>,
}

/// Refine an agent's guardrails (config only — guardrails are presets, not yet enforced).
#[tauri::command]
fn set_agent_guardrails(
    sandbox: String,
    name: String,
    guardrails: GuardrailsDto,
) -> Result<(), String> {
    let mut cfg = load_user_config().unwrap_or_default();
    let g = llmsc_core::config::Guardrails {
        filesystem: guardrails.filesystem,
        network: guardrails.network,
        l3: guardrails.l3,
        llm_budget: guardrails.llm_budget,
        control_plane: guardrails.control_plane,
    };
    if cfg.set_user_guardrails(&sandbox, &name, g) {
        cfg.save(&config::user_config_path())
            .map_err(|e| e.to_string())
    } else {
        Err(format!(
            "agent '{name}' not found in config-managed sandbox '{sandbox}'"
        ))
    }
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
                mem: if running {
                    fmt_mem(s.mem_bytes)
                } else {
                    "—".to_string()
                },
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
                            guardrails: cu
                                .and_then(|c| c.guardrails.clone())
                                .map(to_guardrails_dto),
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
struct AclRuleDto {
    action: String,
    source: String,
    destination: String,
    protocol: String,
    port: String,
    description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkAclDto {
    name: String,
    description: String,
    used_by: usize,
    ingress: Vec<AclRuleDto>,
    egress: Vec<AclRuleDto>,
}

fn acl_rule_dto(r: llmsc_core::incus::AclRule) -> AclRuleDto {
    AclRuleDto {
        action: r.action,
        source: r.source,
        destination: r.destination,
        protocol: r.protocol,
        port: r.port,
        description: r.description,
    }
}

/// Network ACLs (the egress-policy layer) in the project.
#[tauri::command]
fn network_acls() -> Result<Vec<NetworkAclDto>, String> {
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let acls = incus.network_acls().map_err(|e| e.to_string())?;
    Ok(acls
        .into_iter()
        .map(|a| NetworkAclDto {
            name: a.name,
            description: a.description,
            used_by: a.used_by,
            ingress: a.ingress.into_iter().map(acl_rule_dto).collect(),
            egress: a.egress.into_iter().map(acl_rule_dto).collect(),
        })
        .collect())
}

// --- Egress policy (per-container enforcement ring) ---

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EgressPolicyDto {
    /// "deny-all" | "allowlist" | "open"
    posture: String,
    #[serde(default)]
    allow: Vec<String>,
    /// HTTP(S) domain allowlist (L7, enforced by mitmproxy).
    #[serde(default)]
    domains: Vec<String>,
}

fn posture_str(p: llmsc_core::config::EgressPosture) -> String {
    use llmsc_core::config::EgressPosture::*;
    match p {
        DenyAll => "deny-all",
        Allowlist => "allowlist",
        Open => "open",
    }
    .to_string()
}

fn parse_posture(s: &str) -> llmsc_core::config::EgressPosture {
    use llmsc_core::config::EgressPosture::*;
    match s {
        "allowlist" => Allowlist,
        "open" => Open,
        _ => DenyAll,
    }
}

fn to_egress_policy(dto: EgressPolicyDto) -> llmsc_core::config::EgressPolicy {
    llmsc_core::config::EgressPolicy {
        posture: parse_posture(&dto.posture),
        allow: dto.allow,
        domains: dto.domains,
    }
}

/// Read a sandbox's egress policy intent from config. `None` = unmanaged (no ACL).
#[tauri::command]
fn egress_policy(sandbox: String) -> Result<Option<EgressPolicyDto>, String> {
    let cfg = load_user_config()?;
    Ok(cfg
        .sandbox(&sandbox)
        .and_then(|s| s.egress.as_ref())
        .map(|p| EgressPolicyDto {
            posture: posture_str(p.posture),
            allow: p.allow.clone(),
            domains: p.domains.clone(),
        }))
}

/// Write a sandbox's egress policy intent to config (does not enforce — call `apply_egress`).
#[tauri::command]
fn set_egress_policy(sandbox: String, policy: EgressPolicyDto) -> Result<(), String> {
    let mut cfg = load_user_config()?;
    if cfg.set_sandbox_egress(&sandbox, to_egress_policy(policy)) {
        save_user_config(&cfg);
        Ok(())
    } else {
        Err(format!("'{sandbox}' is not config-managed"))
    }
}

/// The compiled Incus ACL for a sandbox's egress policy (for display). `None` if open/unmanaged.
#[tauri::command]
fn egress_acl_preview(sandbox: String) -> Result<Option<NetworkAclDto>, String> {
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let sb = cfg
        .sandbox(&sandbox)
        .ok_or_else(|| format!("'{sandbox}' is not config-managed"))?;
    let ctx = incus.enforce_ctx(&sandbox);
    Ok(
        llmsc_core::enforce::egress_acl(sb, &ctx).map(|a| NetworkAclDto {
            name: a.name,
            description: a.description,
            used_by: a.used_by,
            ingress: a.ingress.into_iter().map(acl_rule_dto).collect(),
            egress: a.egress.into_iter().map(acl_rule_dto).collect(),
        }),
    )
}

/// Enforce a sandbox's egress policy: compile → diff against the live ACL → apply + bind to the
/// nic. Returns the number of ACL ops applied. Open/unmanaged → tear down (unbind + delete the
/// managed ACL) so switching to open actually removes enforcement.
#[tauri::command]
fn apply_egress(app: AppHandle, sandbox: String) -> Result<usize, String> {
    let reporter = EventReporter { app };
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let sb = cfg
        .sandbox(&sandbox)
        .ok_or_else(|| format!("'{sandbox}' is not config-managed"))?;
    incus
        .reconcile_egress(sb, &reporter)
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EgressStatusDto {
    /// Config carries an egress policy (Some).
    managed: bool,
    /// "deny-all" | "allowlist" | "open" | null (unmanaged)
    posture: Option<String>,
    acl_name: String,
    /// The compiled ACL exists in the VM.
    acl_exists: bool,
    /// The nic carries our security.acls binding.
    bound: bool,
    /// Live state matches the compiled intent (nothing to apply).
    in_sync: bool,
}

/// Read the live enforcement status of a sandbox's egress policy (for the GUI badge).
#[tauri::command]
fn egress_status(sandbox: String) -> Result<EgressStatusDto, String> {
    let incus = CliIncus::new(vm_name(), &SystemRunner);
    let acl_name = llmsc_core::enforce::egress_acl_name(&sandbox);
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let sb = cfg.sandbox(&sandbox);
    let policy = sb.and_then(|s| s.egress.as_ref());
    let posture = policy.map(|p| posture_str(p.posture));

    let live = incus.network_acls().unwrap_or_default();
    let acl_exists = live.iter().any(|a| a.name == acl_name);
    let bound = incus
        .instance(&sandbox)
        .ok()
        .map(|i| {
            i.devices.values().any(|d| {
                d.get("security.acls")
                    .map(|v| v.split(',').any(|p| p.trim() == acl_name))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    // in_sync: open/unmanaged → not bound; managed non-open → bound + compiled plan empty.
    let in_sync = match (sb, policy.map(|p| p.posture)) {
        (Some(s), Some(p)) if p != llmsc_core::config::EgressPosture::Open => {
            let ctx = incus.enforce_ctx(&sandbox);
            match llmsc_core::enforce::egress_acl(s, &ctx) {
                Some(desired) => {
                    let live_match = live.iter().find(|a| a.name == acl_name);
                    bound && llmsc_core::enforce::egress_acl_plan(&desired, live_match).is_empty()
                }
                None => !bound,
            }
        }
        _ => !bound,
    };

    Ok(EgressStatusDto {
        managed: policy.is_some(),
        posture,
        acl_name,
        acl_exists,
        bound,
        in_sync,
    })
}

// --- LLM virtual-key budgets (credential-isolation ring) ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VirtualKeyDto {
    key: String,
    assigned_to: String,
    models: String,
    budget: String,
    used: String,
    status: String,
}

/// Per-agent virtual keys compiled from guardrails. Spend is merged best-effort from the live
/// proxy (`/global/spend/keys`): a key with read-back spend shows as `active`, otherwise `planned`.
#[tauri::command]
fn virtual_keys() -> Result<Vec<VirtualKeyDto>, String> {
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    // Best-effort live spend (empty if the proxy is unreachable).
    let usage = LiteLlmDeployer::new(vm_name(), &SystemRunner)
        .key_usage()
        .unwrap_or_default();
    Ok(llmsc_core::enforce::virtual_key_specs(&cfg)
        .into_iter()
        .map(|s| {
            let spend = usage
                .iter()
                .find(|u| u.key_alias == s.key_alias)
                .map(|u| u.spend);
            VirtualKeyDto {
                key: s.key_alias,
                assigned_to: format!("{} @ {}", s.agent, s.sandbox),
                models: if s.models.is_empty() {
                    "all".to_string()
                } else {
                    s.models.join(", ")
                },
                budget: format!("${:.0} / {}", s.max_budget_usd, s.budget_duration),
                used: match spend {
                    Some(v) => format!("${v:.2}"),
                    None => "—".to_string(),
                },
                status: if spend.is_some() { "active" } else { "planned" }.to_string(),
            }
        })
        .collect())
}

/// Set the upstream provider API key — injected ONLY into the LiteLLM container (never stored in
/// llmsc.toml). Credential isolation: real provider keys live only in the service container.
#[tauri::command]
fn set_provider_key(app: AppHandle, provider: String, api_key: String) -> Result<(), String> {
    let reporter = EventReporter { app };
    LiteLlmDeployer::new(vm_name(), &SystemRunner)
        .set_provider_key(&provider, &api_key, &reporter)
        .map_err(|e| e.to_string())
}

/// Sync the compiled virtual keys to the running LiteLLM proxy. Returns the count synced.
/// Requires svc-litellm to be up (provisioned via the Services screen).
#[tauri::command]
fn sync_virtual_keys(app: AppHandle) -> Result<usize, String> {
    let reporter = EventReporter { app };
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let specs = llmsc_core::enforce::virtual_key_specs(&cfg);
    if specs.is_empty() {
        reporter.step("No agents with virtual keys to sync");
        return Ok(0);
    }
    let synced = LiteLlmDeployer::new(vm_name(), &SystemRunner)
        .sync_virtual_keys(&specs, &reporter)
        .map_err(|e| e.to_string())?;
    Ok(synced.len())
}

// --- Tetragon per-UID kernel policies (the kernel ring) ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TetragonPolicyDto {
    name: String,
    agent: String,
    denied_syscalls: Vec<String>,
    egress_note: String,
    fs_note: String,
    read_only: bool,
}

/// Per-agent Tetragon policies compiled from guardrails (the kernel enforcement ring).
#[tauri::command]
fn tetragon_policies(sandbox: String) -> Result<Vec<TetragonPolicyDto>, String> {
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    Ok(llmsc_core::tetragon::sandbox_policies(&cfg, &sandbox)
        .into_iter()
        .map(|p| TetragonPolicyDto {
            name: p.name,
            agent: p.agent,
            denied_syscalls: p.denied_syscalls,
            egress_note: p.egress_note,
            fs_note: p.fs_note,
            read_only: p.read_only,
        })
        .collect())
}

/// The rendered TracingPolicy YAML (DRAFT) for one agent.
#[tauri::command]
fn tetragon_policy_yaml(sandbox: String, agent: String) -> Result<String, String> {
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let g = cfg
        .sandbox(&sandbox)
        .and_then(|s| s.users.iter().find(|u| u.name == agent))
        .and_then(|u| u.guardrails.clone());
    Ok(llmsc_core::tetragon::agent_policy(&sandbox, &agent, g.as_ref()).to_tracing_policy_yaml())
}

/// Load a sandbox's compiled Tetragon policies into the VM. Returns the count applied. Requires
/// Tetragon installed in the VM (scaffold — write+reload is wired, install is follow-up work).
#[tauri::command]
fn apply_tetragon_policies(app: AppHandle, sandbox: String) -> Result<usize, String> {
    let reporter = EventReporter { app };
    let cfg = Config::load_effective().map_err(|e| e.to_string())?;
    let pols = llmsc_core::tetragon::sandbox_policies(&cfg, &sandbox);
    if pols.is_empty() {
        reporter.step("No agent policies to load");
        return Ok(0);
    }
    let applied = llmsc_core::deploy::TetragonDeployer::new(vm_name(), &SystemRunner)
        .apply_policies(&pols, &reporter)
        .map_err(|e| e.to_string())?;
    Ok(applied.len())
}

/// Set/clear `readonly` on a sandbox's workspace mounts (the per-container filesystem backstop).
/// Returns the number of mounts changed.
#[tauri::command]
fn set_workspace_readonly(sandbox: String, readonly: bool) -> Result<usize, String> {
    CliIncus::new(vm_name(), &SystemRunner)
        .set_workspace_readonly(&sandbox, readonly)
        .map_err(|e| e.to_string())
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
            .map(|n| NetworkDto {
                name: n.name,
                kind: n.kind,
                ipv4: n.ipv4,
                nat: n.nat,
                used_by: n.used_by,
            })
            .collect(),
        sandboxes: sandboxes
            .into_iter()
            .map(|s| SandboxNetDto {
                status: if s.status == InstanceStatus::Running {
                    "running"
                } else {
                    "stopped"
                }
                .to_string(),
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
        updated: if i.uploaded.is_empty() {
            "—".to_string()
        } else {
            i.uploaded
        },
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
    let pkgs: Vec<&str> = packages
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .collect();
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
    Ok(container_images(
        incus.images_remote("images").map_err(|e| e.to_string())?,
    ))
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
    cfg.save(&config::user_config_path())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn service_disable(name: String) -> Result<(), String> {
    let mut cfg = load_user_config()?;
    cfg.disable_service(&name);
    cfg.save(&config::user_config_path())
        .map_err(|e| e.to_string())
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
        "mitmproxy" => {
            let d = llmsc_core::deploy::MitmproxyDeployer::new(vm, &SystemRunner);
            d.deploy(&reporter).map_err(|e| e.to_string())?;
            // Load the compiled allowlist (union of sandbox domains).
            let cfg = Config::load_effective().map_err(|e| e.to_string())?;
            d.sync_allowlist(&llmsc_core::enforce::mitmproxy_allowlist(&cfg), &reporter)
                .map_err(|e| e.to_string())
        }
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
    c.save(&config::user_config_path())
        .map_err(|e| e.to_string())?;
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
            instance_config,
            instance_set_config,
            instance_unset_config,
            instance_add_mount,
            instance_remove_device,
            instance_add_profile,
            instance_remove_profile,
            apply_sandbox,
            instance_yaml,
            snapshots,
            snapshot_create,
            snapshot_restore,
            snapshot_delete,
            operator_default,
            add_agent,
            remove_agent,
            set_agent_guardrails,
            profiles,
            incus_profiles,
            storage,
            projects,
            starter_incus_profiles,
            incus_profile_apply,
            reconcile_incus_profiles,
            topology,
            host_resources,
            network_acls,
            egress_policy,
            set_egress_policy,
            egress_acl_preview,
            apply_egress,
            egress_status,
            virtual_keys,
            sync_virtual_keys,
            set_provider_key,
            tetragon_policies,
            tetragon_policy_yaml,
            apply_tetragon_policies,
            set_workspace_readonly,
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
