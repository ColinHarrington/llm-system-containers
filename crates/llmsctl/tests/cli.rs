//! Black-box CLI smoke tests for `llmsctl` (M0).

use assert_cmd::Command;
use predicates::str::contains;

fn llmsctl() -> Command {
    Command::cargo_bin("llmsctl").unwrap()
}

#[test]
fn help_lists_subcommands() {
    llmsctl()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("up"))
        .stdout(contains("status"));
}

#[test]
fn init_prints_default_config() {
    // `init` is deterministic (no VM/IO), so it's the safe command to smoke-test here;
    // up/down/status touch the environment and are covered by core unit + future integration tests.
    llmsctl()
        .arg("init")
        .assert()
        .success()
        .stdout(contains("[vm]"))
        .stdout(contains("llmsc"));
}

#[test]
fn unknown_subcommand_fails() {
    llmsctl().arg("bogus").assert().failure();
}

// A `[vm]` with a name that doesn't exist, so `doctor` finds the VM not-running and skips the
// slow live Incus checks — keeping the test fast and deterministic.
const VM_TOML: &str =
    "[vm]\nname = \"llmsctl-doctor-test\"\ncpus = 4\nmemory_gib = 8\ndisk_gib = 100\n\n";

fn in_project(sandboxes: &str, args: &[&str]) -> assert_cmd::assert::Assert {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "llmsctl-cli-{}-{}-{}",
        std::process::id(),
        args.join("_"),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("llmsc.toml"), format!("{VM_TOML}{sandboxes}")).unwrap();
    let out = llmsctl().current_dir(&dir).args(args).assert();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

#[test]
fn status_rejects_unsupported_deployment_target() {
    // `mode` is a top-level key, so it must precede the [vm] table.
    let toml = "mode = \"remote\"\n[vm]\nname = \"x\"\ncpus = 4\nmemory_gib = 8\ndisk_gib = 100\n";
    let dir = std::env::temp_dir().join(format!("llmsctl-cli-{}-moderemote", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("llmsc.toml"), toml).unwrap();
    let out = llmsctl().current_dir(&dir).args(["status"]).assert();
    let _ = std::fs::remove_dir_all(&dir);
    out.failure().stderr(contains("not supported"));
}

#[test]
fn config_valid_reports_ok() {
    let toml = "[[sandbox]]\nname = \"web\"\nimage = \"images:alpine/3.21\"\n";
    in_project(toml, &["config"])
        .success()
        .stdout(contains("config valid"))
        .stdout(contains("target:"));
}

#[test]
fn config_invalid_reports_issues() {
    // Two sandboxes with the same name → validation flags a duplicate.
    let toml = "[[sandbox]]\nname = \"dup\"\nimage = \"images:alpine/3.21\"\n\n[[sandbox]]\nname = \"dup\"\nimage = \"images:alpine/3.21\"\n";
    in_project(toml, &["config"])
        .failure()
        .stderr(contains("duplicate sandbox name 'dup'"));
}

#[test]
fn doctor_reports_target_and_remote_display() {
    let toml = "[[sandbox]]\nname = \"web-agent-01\"\nimage = \"images:alpine/3.21\"\ndisplay = \"xpra\"\n";
    in_project(toml, &["doctor"])
        .success()
        .stdout(contains("Target: vm"))
        .stdout(contains("Remote display:"))
        .stdout(contains("web-agent-01"))
        .stdout(contains("xpra"));
}
