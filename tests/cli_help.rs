use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cli_help_lists_core_commands() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("daemon"))
        .stdout(predicate::str::contains("library"))
        .stdout(predicate::str::contains("playlist"))
        .stdout(predicate::str::contains("queue"))
        .stdout(predicate::str::contains("tui"))
        .stdout(predicate::str::contains("status"));
}
