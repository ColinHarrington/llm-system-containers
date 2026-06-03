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
fn status_runs() {
    llmsctl()
        .arg("status")
        .assert()
        .success()
        .stdout(contains("not yet implemented"));
}

#[test]
fn unknown_subcommand_fails() {
    llmsctl().arg("bogus").assert().failure();
}
