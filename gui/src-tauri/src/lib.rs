//! Tauri command layer for the llmsc GUI.
//!
//! These commands are thin wrappers over `llmsc-core` (the same logic the CLIs use) — they
//! shell out to Lima/Incus on the host. Long operations report progress with a silent reporter
//! for now (streaming progress to the GUI is a follow-up).

use llmsc_core::bootstrap::IncusBootstrap;
use llmsc_core::config::{self, Config, Sandbox};
use llmsc_core::incus::{CliIncus, IncusClient, InstanceStatus};
use llmsc_core::process::SystemRunner;
use llmsc_core::progress::Reporter;
use llmsc_core::service;
use llmsc_core::vm::{LimaVmDriver, VmDriver, VmStatus};
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
    let items = incus.list().map_err(|e| e.to_string())?;
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
fn sandbox_launch(app: AppHandle, name: String, image: String, nesting: bool) -> Result<(), String> {
    let reporter = EventReporter { app };
    let runner = SystemRunner;
    let incus = CliIncus::new(vm_name(), &runner);
    let spec = Sandbox { name, image, nesting, users: vec![] };
    incus.launch(&spec, &reporter).map_err(|e| e.to_string())
}

#[tauri::command]
fn sandbox_rm(name: String) -> Result<(), String> {
    let runner = SystemRunner;
    let incus = CliIncus::new(vm_name(), &runner);
    incus.delete(&name).map_err(|e| e.to_string())
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

#[derive(Deserialize)]
struct SetupCfg {
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
            service_list,
            service_enable,
            service_disable,
            platform_init
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
