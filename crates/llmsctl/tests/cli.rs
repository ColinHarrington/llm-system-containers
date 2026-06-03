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
