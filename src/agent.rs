use crate::{
    message::{Message, ToolCall},
    provider::ModelProvider,
    tool::ToolDefinition,
};

#[derive(Debug)]
pub(crate) enum TurnOutcome {
    FinalText(String),
    ToolCalls(Vec<ToolCall>),
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
        TurnOutcome::ToolCalls(response.tool_calls.clone())
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
            Ok(Message {
                role: String::from("assistant"),
                content: None,
                tool_calls: vec![ToolCall {
                    id: String::from("call_123"),
                    kind: String::from("function"),
                    function: FunctionCall {
                        name: String::from("bash"),
                        arguments: String::from(r#"{"command":"pwd"}"#),
                    },
                }],
                tool_call_id: None,
            })
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
                assert_eq!(tool_calls[0].id, "call_123");
                assert_eq!(tool_calls[0].function.name, "bash");
            }
            TurnOutcome::FinalText(_) => panic!("expected tool calls"),
        }
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[2].tool_calls.len(), 1);
    }
}
