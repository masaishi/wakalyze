use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

#[test]
fn config_path_outputs_path() {
    cargo_bin_cmd!("wakalyze")
        .args(["config", "path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("config.json"));
}

#[test]
fn config_set_then_show() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("wakalyze");

    cargo_bin_cmd!("wakalyze")
        .env("XDG_CONFIG_HOME", dir.path())
        .args(["config", "set", "--user", "testuser"])
        .assert()
        .success();

    assert!(config_dir.join("config.json").exists());

    cargo_bin_cmd!("wakalyze")
        .env("XDG_CONFIG_HOME", dir.path())
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("testuser"));
}

#[test]
fn missing_auth_shows_error() {
    cargo_bin_cmd!("wakalyze")
        .env_remove("WAKAPI_KEY")
        .env("WAKAPI_USER", "testuser")
        .env("XDG_CONFIG_HOME", tempfile::tempdir().unwrap().path())
        .args(["analyze", "2026/02"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing auth"));
}

#[test]
fn missing_user_shows_error() {
    cargo_bin_cmd!("wakalyze")
        .env_remove("WAKAPI_USER")
        .env("WAKAPI_KEY", "sometoken")
        .env("XDG_CONFIG_HOME", tempfile::tempdir().unwrap().path())
        .args(["analyze", "2026/02"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing user"));
}

#[test]
fn help_flag_works() {
    cargo_bin_cmd!("wakalyze")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("wakalyze"));
}

#[test]
fn implicit_analyze_subcommand() {
    // "wakalyze 2026/02" should work the same as "wakalyze analyze 2026/02"
    // (will fail due to missing auth but should not fail due to parsing)
    cargo_bin_cmd!("wakalyze")
        .env_remove("WAKAPI_KEY")
        .env("WAKAPI_USER", "testuser")
        .env("XDG_CONFIG_HOME", tempfile::tempdir().unwrap().path())
        .arg("2026/02")
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing auth"));
}
