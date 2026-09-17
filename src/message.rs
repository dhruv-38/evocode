use serde::{Deserialize, Serialize};

const SYSTEM_PROMPT: &str = "You are a helpful coding assistant.";

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

pub(crate) fn initial_messages(prompt: String) -> Vec<Message> {
    vec![
        Message::text("system", String::from(SYSTEM_PROMPT)),
        Message::text("user", prompt),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_messages_contain_system_instruction_and_user_prompt() {
        let messages = initial_messages(String::from("Fix the parser"));

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content.as_deref(), Some(SYSTEM_PROMPT));
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
