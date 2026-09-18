use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Message {
    pub(crate) role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tool_calls: Vec<ToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_call_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ToolCall {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) function: FunctionCall,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct FunctionCall {
    pub(crate) name: String,
    pub(crate) arguments: String,
}

impl Message {
    pub(crate) fn text(role: &str, content: String) -> Self {
        Self {
            role: String::from(role),
            content: Some(content),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub(crate) fn tool_result(tool_call_id: String, content: String) -> Self {
        Self {
            role: String::from("tool"),
            content: Some(content),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id),
        }
    }
}

pub(crate) fn conversation_messages(working_directory: &Path) -> Vec<Message> {
    vec![Message::text("system", system_prompt(working_directory))]
}

fn system_prompt(working_directory: &Path) -> String {
    format!(
        r#"You are Evocode, a terminal coding agent helping the user understand and modify a local project.

Current working directory: {working_directory:?}

Operating rules:
- Use the bash tool to inspect files, run commands, edit code, and verify work when needed.
- Treat tool results as authoritative. Never claim a command succeeded without seeing its result.
- Work inside the current working directory unless the user explicitly asks otherwise.
- Inspect relevant files before editing and preserve unrelated user changes.
- Prefer small, focused changes that directly address the user's request.
- Run relevant checks after making changes and report any checks you could not run.
- Avoid destructive commands unless the user explicitly requests them.
- Each bash call starts in the current working directory; directory changes do not persist between calls.
- Avoid interactive commands because bash tool calls cannot answer their prompts.
- Long command output may be truncated in the middle. Use narrower commands when more detail is needed.
- When no tool is needed, answer the user directly and concisely."#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_contains_system_instruction_and_user_prompt() {
        let mut messages = conversation_messages(Path::new("/work/project"));
        messages.push(Message::text("user", String::from("Fix the parser")));

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        let system_prompt = messages[0]
            .content
            .as_deref()
            .expect("system message should contain text");
        assert!(system_prompt.contains("terminal coding agent"));
        assert!(system_prompt.contains(r#"Current working directory: "/work/project""#));
        assert!(system_prompt.contains("Use the bash tool"));
        assert!(system_prompt.contains("directory changes do not persist"));
        assert!(system_prompt.contains("output may be truncated"));
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content.as_deref(), Some("Fix the parser"));
        assert!(messages.iter().all(|message| message.tool_calls.is_empty()));
        assert!(
            messages
                .iter()
                .all(|message| message.tool_call_id.is_none())
        );
    }

    #[test]
    fn tool_result_references_original_call() {
        let message = Message::tool_result(String::from("call_123"), String::from("Exit code: 0"));
        let value = serde_json::to_value(message).expect("message should serialize");

        assert_eq!(value["role"], "tool");
        assert_eq!(value["tool_call_id"], "call_123");
        assert_eq!(value["content"], "Exit code: 0");
        assert!(value.get("tool_calls").is_none());
    }
}
