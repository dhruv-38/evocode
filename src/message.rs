use serde::{Deserialize, Serialize};

const SYSTEM_PROMPT: &str = "You are a helpful coding assistant.";

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Message {
    pub(crate) role: String,
    pub(crate) content: String,
}

pub(crate) fn initial_messages(prompt: String) -> Vec<Message> {
    vec![
        Message {
            role: String::from("system"),
            content: String::from(SYSTEM_PROMPT),
        },
        Message {
            role: String::from("user"),
            content: prompt,
        },
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
        assert_eq!(messages[0].content, SYSTEM_PROMPT);
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content, "Fix the parser");
    }
}
