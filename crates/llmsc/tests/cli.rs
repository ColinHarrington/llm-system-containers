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
fn ls_runs() {
    llmsc()
        .arg("ls")
        .assert()
        .success()
        .stdout(contains("not yet implemented"));
}

#[test]
fn unknown_subcommand_fails() {
    llmsc().arg("bogus").assert().failure();
}
