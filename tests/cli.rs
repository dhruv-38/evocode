use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static TEST_DIRECTORY_ID: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let id = TEST_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rust-terminal-agent-cli-test-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test directory should be created");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("test directory should be removed");
    }
}

fn run_agent(directory: &Path, arguments: &[&str], input: &str, api_key: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rust-terminal-agent"));
    command
        .args(arguments)
        .current_dir(directory)
        .env_remove("GROQ_API_KEY")
        .env_remove("GROQ_MODEL")
        .env_remove("AGENT_MAX_TURNS")
        .env_remove("AGENT_BASH_TIMEOUT_SECS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(api_key) = api_key {
        command.env("GROQ_API_KEY", api_key);
    }

    let mut child = command.spawn().expect("agent should start");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(input.as_bytes())
        .expect("input should be written");
    child.wait_with_output().expect("agent should exit")
}

fn output_text(output: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn exits_with_an_error_when_the_api_key_is_missing() {
    let directory = TestDirectory::new();

    let output = run_agent(&directory.0, &[], "", None);
    let (stdout, stderr) = output_text(&output);

    assert!(!output.status.success());
    assert!(stdout.is_empty());
    assert!(stderr.contains("Error: GROQ_API_KEY is not set"));
}

#[test]
fn prints_configured_values_without_exposing_the_api_key() {
    let directory = TestDirectory::new();
    let mut command = Command::new(env!("CARGO_BIN_EXE_rust-terminal-agent"));
    let output = command
        .arg("/config")
        .current_dir(&directory.0)
        .env("GROQ_API_KEY", "never-print-this-key")
        .env("GROQ_MODEL", "test/model")
        .env("AGENT_MAX_TURNS", "7")
        .env("AGENT_BASH_TIMEOUT_SECS", "15")
        .env_remove("RUST_LOG")
        .output()
        .expect("agent should run");
    let (stdout, stderr) = output_text(&output);

    assert!(output.status.success());
    assert!(stderr.is_empty());
    assert!(stdout.contains("Model: test/model"));
    assert!(stdout.contains("Maximum agent turns: 7"));
    assert!(stdout.contains("Bash timeout: 15 seconds"));
    assert!(!stdout.contains("never-print-this-key"));
}

#[test]
fn local_commands_and_session_round_trip_work_without_the_api() {
    let directory = TestDirectory::new();
    let input = "/help\n/history\n/save\n/clear\n/load\n/history\n/not-a-command\n/exit\n";

    let output = run_agent(&directory.0, &[], input, Some("offline-test-key"));
    let (stdout, stderr) = output_text(&output);

    assert!(output.status.success());
    assert!(stderr.is_empty());
    assert!(stdout.contains("/history      Show this conversation"));
    assert_eq!(stdout.matches("No conversation history.").count(), 2);
    assert!(stdout.contains("Session saved to"));
    assert!(stdout.contains("Conversation cleared."));
    assert!(stdout.contains("Session loaded from"));
    assert!(stdout.contains("Unknown command: /not-a-command"));

    let session_path = directory.0.join(".rust-terminal-agent-session.json");
    let session = fs::read_to_string(session_path).expect("session should have been saved");
    let session: serde_json::Value =
        serde_json::from_str(&session).expect("session should contain JSON");
    assert_eq!(session["version"], 1);
    assert_eq!(session["messages"].as_array().map(Vec::len), Some(0));
}
