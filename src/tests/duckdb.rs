use super::*;

pub(super) fn run_duckdb(sql: &str) -> String {
    let mut command = Command::new("build/release/duckdb");
    command.env("TZ", "UTC");
    run_duckdb_command(&mut command, sql)
}

pub(super) fn run_duckdb_with_env(sql: &str, name: &str, value: &str) -> String {
    let mut command = Command::new("build/release/duckdb");
    command.env("TZ", "UTC");
    command.env(name, value);
    run_duckdb_command(&mut command, sql)
}

pub(super) fn run_duckdb_command(command: &mut Command, sql: &str) -> String {
    let output = command
        .args(["-batch", "-csv", ":memory:", "-c", sql])
        .output()
        .expect("failed to run build/release/duckdb; run make release first");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "duckdb failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout,
        stderr
    );
    format!("{stdout}{stderr}")
}

pub(super) fn run_duckdb_command_after_setup(setup: &str, sql: &str) -> (bool, String) {
    let output = Command::new("build/release/duckdb")
        .args([
            "-batch", "-bail", "-csv", "-cmd", setup, ":memory:", "-c", sql,
        ])
        .output()
        .expect("failed to run build/release/duckdb; run make release first");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    (output.status.success(), format!("{stdout}{stderr}"))
}

pub(super) fn extension_path() -> &'static str {
    let path = "build/release/extension/superhuman_docs/superhuman_docs.duckdb_extension";
    assert!(
        Path::new(path).exists(),
        "{path} does not exist; run make release first"
    );
    path
}

pub(super) fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub(super) fn sql_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
