use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cli_help_lists_core_commands_and_examples() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("daemon"))
        .stdout(predicate::str::contains("library"))
        .stdout(predicate::str::contains("playlist"))
        .stdout(predicate::str::contains("queue"))
        .stdout(predicate::str::contains("tui"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains(
            "Daemon-backed local music library manager",
        ))
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("melo status"));
}

#[test]
fn library_help_describes_scan_and_organize_boundaries() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("library").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Scan, inspect, and organize library content",
        ))
        .stdout(predicate::str::contains("melo library scan D:/Music"))
        .stdout(predicate::str::contains("melo library organize --preview"));
}

#[test]
fn playlist_help_includes_maintenance_examples() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("playlist").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Maintain direct-open ephemeral playlists",
        ))
        .stdout(predicate::str::contains(
            "melo playlist promote D:/Music/blue-bird.mp3 Favorites",
        ))
        .stdout(predicate::str::contains("melo playlist cleanup"));
}

#[test]
fn playlist_help_mentions_promote_and_cleanup() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("playlist").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("promote"))
        .stdout(predicate::str::contains("cleanup"));
}

#[test]
fn db_help_includes_maintenance_examples() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("db").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Inspect and maintain the Melo database",
        ))
        .stdout(predicate::str::contains("melo db doctor"))
        .stdout(predicate::str::contains("melo db backup ./backup/melo.db"));
}

#[test]
fn cli_help_lists_structured_player_command() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("player"))
        .stdout(predicate::str::contains("queue"))
        .stdout(predicate::str::contains("playlist"));
}

#[test]
fn daemon_help_lists_management_commands_and_flags() {
    let mut cmd = Command::cargo_bin("melo").unwrap();
    cmd.arg("daemon").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("restart"))
        .stdout(predicate::str::contains("logs"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("--json"))
        .stdout(predicate::str::contains("--verbose"));
}
