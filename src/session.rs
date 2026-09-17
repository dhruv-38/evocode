use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::message::Message;

pub(crate) const DEFAULT_SESSION_FILE: &str = ".rust-terminal-agent-session.json";
const SESSION_VERSION: u32 = 1;
const MAX_SESSION_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
struct SavedSession {
    version: u32,
    messages: Vec<Message>,
}

pub(crate) fn save_session(path: &Path, messages: &[Message]) -> Result<(), String> {
    let session = SavedSession {
        version: SESSION_VERSION,
        messages: messages
            .iter()
            .filter(|message| message.role != "system")
            .cloned()
            .collect(),
    };
    let contents = serde_json::to_string_pretty(&session)
        .map_err(|error| format!("Could not serialize session: {error}"))?;
    if contents.len() as u64 > MAX_SESSION_BYTES {
        return Err(format!(
            "Session is too large to save (maximum {} MiB)",
            MAX_SESSION_BYTES / 1024 / 1024
        ));
    }

    fs::write(path, contents)
        .map_err(|error| format!("Could not save session to {}: {error}", path.display()))
}

pub(crate) fn load_session(path: &Path) -> Result<Vec<Message>, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Could not open session {}: {error}", path.display()))?;
    if metadata.len() > MAX_SESSION_BYTES {
        return Err(format!(
            "Session is too large to load (maximum {} MiB)",
            MAX_SESSION_BYTES / 1024 / 1024
        ));
    }

    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Could not read session {}: {error}", path.display()))?;
    let session = serde_json::from_str::<SavedSession>(&contents)
        .map_err(|error| format!("Could not parse session {}: {error}", path.display()))?;
    if session.version != SESSION_VERSION {
        return Err(format!(
            "Unsupported session version {} (expected {SESSION_VERSION})",
            session.version
        ));
    }
    if session
        .messages
        .iter()
        .any(|message| message.role == "system")
    {
        return Err(String::from(
            "Session contains an unexpected system message",
        ));
    }

    Ok(session.messages)
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    static TEST_DIRECTORY_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = TEST_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rust-terminal-agent-session-test-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("test directory should be created");
            Self(path)
        }

        fn file(&self) -> PathBuf {
            self.0.join("session.json")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("test directory should be removed");
        }
    }

    #[test]
    fn saves_and_loads_conversation_messages() {
        let directory = TestDirectory::new();
        let path = directory.file();
        let messages = vec![
            Message::text("system", String::from("old instructions")),
            Message::text("user", String::from("hello")),
            Message::text("assistant", String::from("hi")),
        ];

        save_session(&path, &messages).expect("session should save");
        let loaded = load_session(&path).expect("session should load");

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].role, "user");
        assert_eq!(loaded[0].content.as_deref(), Some("hello"));
        assert_eq!(loaded[1].role, "assistant");
        assert_eq!(loaded[1].content.as_deref(), Some("hi"));
    }

    #[test]
    fn rejects_unsupported_session_versions() {
        let directory = TestDirectory::new();
        let path = directory.file();
        fs::write(&path, r#"{"version":2,"messages":[]}"#).expect("test session should be written");

        let error = load_session(&path).expect_err("session should be rejected");

        assert_eq!(error, "Unsupported session version 2 (expected 1)");
    }

    #[test]
    fn rejects_saved_system_messages() {
        let directory = TestDirectory::new();
        let path = directory.file();
        fs::write(
            &path,
            r#"{"version":1,"messages":[{"role":"system","content":"unsafe"}]}"#,
        )
        .expect("test session should be written");

        let error = load_session(&path).expect_err("session should be rejected");

        assert_eq!(error, "Session contains an unexpected system message");
    }
}
