use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use std::fs;

fn setup_test_project(path: &std::path::Path) {
    fs::write(path.join("spora.toml"), r#"
        [project]
        name = "test-project"
        lang = "java"
        version = "0.1.0"
        [runtime]
        vendor = "temurin"
        version = "21"
    "#).unwrap();
}

#[test]
fn test_spora_commands_with_assert_cmd() {
    let temp = tempfile::tempdir().unwrap();
    let p = temp.path();
    setup_test_project(p);

    for cmd_name in ["init", "fetch", "build", "clean"] {
        let mut cmd = Command::cargo_bin("spora").unwrap();

        cmd.env("JAVA_TOOL_OPTIONS", "-Duser.language=en");
        cmd.env("LC_ALL", "C");
        
        cmd.current_dir(p)
            .arg(cmd_name)
            .assert()
            .success()
            .stdout(predicate::str::contains("Success").or(predicate::str::is_empty())); 
    }
}