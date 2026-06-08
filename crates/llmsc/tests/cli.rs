//! Black-box CLI smoke tests for `llmsc` (M0).

use assert_cmd::Command;
use predicates::str::contains;

fn llmsc() -> Command {
    Command::cargo_bin("llmsc").unwrap()
}

#[test]
fn help_lists_subcommands() {
    llmsc()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("launch"))
        .stdout(contains("shell"));
}

#[test]
fn cp_rejects_two_host_paths() {
    // Both args are host paths → error before any Incus I/O (deterministic, no VM needed).
    llmsc()
        .args(["cp", "a", "b"])
        .assert()
        .failure()
        .stderr(contains("container ref"));
}

#[test]
fn cp_rejects_container_to_container() {
    llmsc()
        .args(["cp", "web:/a", "other:/b"])
        .assert()
        .failure()
        .stderr(contains("not supported yet"));
}

#[test]
fn unknown_subcommand_fails() {
    llmsc().arg("bogus").assert().failure();
}

/// A minimal valid `[vm]` block (required by the config) the sandbox fixtures are appended to.
const VM_TOML: &str = "[vm]\nname = \"llmsc\"\ncpus = 4\nmemory_gib = 8\ndisk_gib = 100\n\n";

/// Run `llmsc <args>` in a throwaway dir holding `VM_TOML + sandboxes` as `llmsc.toml` (so
/// `load_effective` picks up the project config), returning the assert handle.
fn in_project(sandboxes: &str, args: &[&str]) -> assert_cmd::assert::Assert {
    let dir = std::env::temp_dir().join(format!(
        "llmsc-cli-{}-{}",
        std::process::id(),
        args.join("_")
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("llmsc.toml"), format!("{VM_TOML}{sandboxes}")).unwrap();
    let out = llmsc().current_dir(&dir).args(args).assert();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

#[test]
fn display_shows_xpra_recipe() {
    let toml = "[[sandbox]]\nname = \"web-agent-01\"\nimage = \"images:alpine/3.21\"\ndisplay = \"xpra\"\n";
    in_project(toml, &["display", "web-agent-01"])
        .success()
        .stdout(contains("display: xpra"))
        .stdout(contains("xpra attach tcp://127.0.0.1:14500"));
}

#[test]
fn launch_rejects_unknown_display_method() {
    // The --display value is validated before any Incus I/O, so this is deterministic (no VM).
    llmsc()
        .args(["launch", "x", "--display", "bogus"])
        .assert()
        .failure()
        .stderr(contains("unknown display method"));
}

#[test]
fn launch_help_lists_display_flag() {
    llmsc()
        .args(["launch", "--help"])
        .assert()
        .success()
        .stdout(contains("--display"));
}

#[test]
fn rejects_unsupported_deployment_target() {
    // `local` is now wired (runs incus directly); `remote` is still reserved. `mode` is a
    // top-level key, so it must precede the [vm] table.
    let toml =
        "mode = \"remote\"\n[vm]\nname = \"llmsc\"\ncpus = 4\nmemory_gib = 8\ndisk_gib = 100\n";
    let dir = std::env::temp_dir().join(format!("llmsc-cli-{}-moderemote", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("llmsc.toml"), toml).unwrap();
    let out = llmsc().current_dir(&dir).args(["ls"]).assert();
    let _ = std::fs::remove_dir_all(&dir);
    out.failure().stderr(contains("not supported"));
}

#[test]
fn display_none_when_unset() {
    let toml = "[[sandbox]]\nname = \"plain\"\nimage = \"images:alpine/3.21\"\n";
    in_project(toml, &["display", "plain"])
        .success()
        .stdout(contains("display: none"));
}
