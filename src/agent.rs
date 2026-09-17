use crate::{
    bash::{BashCall, parse_bash_call},
    message::Message,
    provider::ModelProvider,
    tool::ToolDefinition,
};

#[derive(Debug)]
pub(crate) enum TurnOutcome {
    FinalText(String),
    ToolCalls(Vec<BashCall>),
}

pub(crate) async fn run_turn<P: ModelProvider>(
    provider: &P,
    messages: &mut Vec<Message>,
    tools: &[ToolDefinition],
) -> Result<TurnOutcome, String> {
    let response = provider.send(messages, tools).await?;
    let outcome = if response.tool_calls.is_empty() {
        let content = response
            .content
            .clone()
            .ok_or_else(|| String::from("The model response contained no text or tool calls"))?;
        TurnOutcome::FinalText(content)
    } else {
        let tool_calls = response
            .tool_calls
            .iter()
            .map(parse_bash_call)
            .collect::<Result<Vec<_>, _>>()?;
        TurnOutcome::ToolCalls(tool_calls)
    };
    messages.push(response);

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        message::{FunctionCall, initial_messages},
        tool::default_tools,
    };

    struct FakeProvider;

    struct FailingProvider;

    struct ToolCallingProvider;

    struct InvalidToolCallingProvider;

    fn tool_call_message(arguments: &str) -> Message {
        Message {
            role: String::from("assistant"),
            content: None,
            tool_calls: vec![crate::message::ToolCall {
                id: String::from("call_123"),
                kind: String::from("function"),
                function: FunctionCall {
                    name: String::from("bash"),
                    arguments: String::from(arguments),
                },
            }],
            tool_call_id: None,
        }
    }

    impl ModelProvider for FakeProvider {
        async fn send(
            &self,
            messages: &[Message],
            tools: &[ToolDefinition],
        ) -> Result<Message, String> {
            assert_eq!(messages.len(), 2);
            assert_eq!(tools.len(), 1);
            assert_eq!(tools[0].function.name, "bash");

            Ok(Message::text("assistant", String::from("fake response")))
        }
    }

    impl ModelProvider for FailingProvider {
        async fn send(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
        ) -> Result<Message, String> {
            Err(String::from("provider failed"))
        }
    }

    impl ModelProvider for ToolCallingProvider {
        async fn send(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
        ) -> Result<Message, String> {
            Ok(tool_call_message(r#"{"command":"pwd"}"#))
        }
    }

    impl ModelProvider for InvalidToolCallingProvider {
        async fn send(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
        ) -> Result<Message, String> {
            Ok(tool_call_message("not json"))
        }
    }

    #[tokio::test]
    async fn appends_assistant_response_to_history() {
        let provider = FakeProvider;
        let mut messages = initial_messages(String::from("test prompt"));
        let tools = default_tools();

        let outcome = run_turn(&provider, &mut messages, &tools)
            .await
            .expect("turn should succeed");

        match outcome {
            TurnOutcome::FinalText(response) => assert_eq!(response, "fake response"),
            TurnOutcome::ToolCalls(_) => panic!("expected final text"),
        }
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[2].content.as_deref(), Some("fake response"));
    }

    #[tokio::test]
    async fn leaves_history_unchanged_when_provider_fails() {
        let provider = FailingProvider;
        let mut messages = initial_messages(String::from("test prompt"));
        let tools = default_tools();

        let error = run_turn(&provider, &mut messages, &tools)
            .await
            .expect_err("turn should fail");

        assert_eq!(error, "provider failed");
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn returns_tool_calls_and_appends_assistant_message() {
        let provider = ToolCallingProvider;
        let mut messages = initial_messages(String::from("show the current directory"));
        let tools = default_tools();

        let outcome = run_turn(&provider, &mut messages, &tools)
            .await
            .expect("turn should succeed");

        match outcome {
            TurnOutcome::ToolCalls(tool_calls) => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].tool_call_id, "call_123");
                assert_eq!(tool_calls[0].command, "pwd");
            }
            TurnOutcome::FinalText(_) => panic!("expected tool calls"),
        }
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[2].tool_calls.len(), 1);
    }

    #[tokio::test]
    async fn rejects_invalid_tool_arguments_without_changing_history() {
        let provider = InvalidToolCallingProvider;
        let mut messages = initial_messages(String::from("show the current directory"));
        let tools = default_tools();

        let error = run_turn(&provider, &mut messages, &tools)
            .await
            .expect_err("turn should fail");

        assert!(error.starts_with("Invalid Bash tool arguments:"));
        assert_eq!(messages.len(), 2);
    }
}
