use serde::Deserialize;

use crate::{message::ToolCall, tool::BASH_TOOL_NAME};

#[derive(Debug)]
pub(crate) struct BashCall {
    pub(crate) tool_call_id: String,
    pub(crate) command: String,
}

#[derive(Deserialize)]
struct BashArguments {
    command: String,
}

pub(crate) fn parse_bash_call(tool_call: &ToolCall) -> Result<BashCall, String> {
    if tool_call.kind != "function" {
        return Err(format!("Unsupported tool call type: {}", tool_call.kind));
    }
    if tool_call.function.name != BASH_TOOL_NAME {
        return Err(format!("Unknown tool: {}", tool_call.function.name));
    }

    let arguments = serde_json::from_str::<BashArguments>(&tool_call.function.arguments)
        .map_err(|error| format!("Invalid Bash tool arguments: {error}"))?;
    if arguments.command.trim().is_empty() {
        return Err(String::from("Bash command cannot be empty"));
    }

    Ok(BashCall {
        tool_call_id: tool_call.id.clone(),
        command: arguments.command,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::FunctionCall;

    fn tool_call(name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: String::from("call_123"),
            kind: String::from("function"),
            function: FunctionCall {
                name: String::from(name),
                arguments: String::from(arguments),
            },
        }
    }

    #[test]
    fn parses_valid_bash_arguments() {
        let parsed = parse_bash_call(&tool_call("bash", r#"{"command":"pwd"}"#))
            .expect("arguments should parse");

        assert_eq!(parsed.tool_call_id, "call_123");
        assert_eq!(parsed.command, "pwd");
    }

    #[test]
    fn rejects_unknown_tool() {
        let error = parse_bash_call(&tool_call("read_file", r#"{"command":"pwd"}"#))
            .expect_err("unknown tool should fail");

        assert_eq!(error, "Unknown tool: read_file");
    }

    #[test]
    fn rejects_malformed_arguments() {
        let error = parse_bash_call(&tool_call("bash", "not json"))
            .expect_err("invalid arguments should fail");

        assert!(error.starts_with("Invalid Bash tool arguments:"));
    }

    #[test]
    fn rejects_empty_command() {
        let error = parse_bash_call(&tool_call("bash", r#"{"command":"  "}"#))
            .expect_err("empty command should fail");

        assert_eq!(error, "Bash command cannot be empty");
    }
}
