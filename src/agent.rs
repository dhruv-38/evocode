use crate::{
    bash::{BashCall, BashExecutor, parse_bash_call},
    message::Message,
    provider::ModelProvider,
    tool::ToolDefinition,
};

pub(crate) const DEFAULT_MAX_TURNS: usize = 10;

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

pub(crate) async fn run_agent<P: ModelProvider, E: BashExecutor>(
    provider: &P,
    executor: &E,
    messages: &mut Vec<Message>,
    tools: &[ToolDefinition],
    max_turns: usize,
) -> Result<String, String> {
    for _ in 0..max_turns {
        match run_turn(provider, messages, tools).await? {
            TurnOutcome::FinalText(response) => return Ok(response),
            TurnOutcome::ToolCalls(tool_calls) => {
                for tool_call in tool_calls {
                    let result = executor.execute(&tool_call).await;
                    messages.push(result);
                }
            }
        }
    }

    Err(format!("Agent reached maximum turn limit of {max_turns}"))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::{
        message::{FunctionCall, conversation_messages},
        tool::default_tools,
    };

    struct FakeProvider;

    struct FailingProvider;

    struct ToolCallingProvider;

    struct InvalidToolCallingProvider;

    struct LoopProvider {
        requests: AtomicUsize,
    }

    struct AlwaysToolProvider {
        requests: AtomicUsize,
    }

    struct FakeExecutor;

    fn initial_messages(prompt: String) -> Vec<Message> {
        let mut messages = conversation_messages();
        messages.push(Message::text("user", prompt));
        messages
    }

    fn tool_call_message(arguments: &str) -> Message {
        tool_call_message_with_id("call_123", arguments)
    }

    fn tool_call_message_with_id(id: &str, arguments: &str) -> Message {
        Message {
            role: String::from("assistant"),
            content: None,
            tool_calls: vec![crate::message::ToolCall {
                id: String::from(id),
                kind: String::from("function"),
                function: FunctionCall {
                    name: String::from("bash"),
                    arguments: String::from(arguments),
                },
            }],
            tool_call_id: None,
        }
    }

    impl BashExecutor for FakeExecutor {
        async fn execute(&self, call: &BashCall) -> Message {
            Message::tool_result(
                call.tool_call_id.clone(),
                String::from("Exit code: 0\nstdout:\n/home/test"),
            )
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

    impl ModelProvider for LoopProvider {
        async fn send(
            &self,
            messages: &[Message],
            _tools: &[ToolDefinition],
        ) -> Result<Message, String> {
            match self.requests.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    assert_eq!(messages.len(), 2);
                    Ok(tool_call_message(r#"{"command":"pwd"}"#))
                }
                1 => {
                    assert_eq!(messages.len(), 4);
                    assert_eq!(messages[2].role, "assistant");
                    assert_eq!(messages[3].role, "tool");
                    assert_eq!(messages[3].tool_call_id.as_deref(), Some("call_123"));
                    Ok(Message::text(
                        "assistant",
                        String::from("You are in /home/test"),
                    ))
                }
                _ => panic!("agent made too many provider requests"),
            }
        }
    }

    impl ModelProvider for AlwaysToolProvider {
        async fn send(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
        ) -> Result<Message, String> {
            let request = self.requests.fetch_add(1, Ordering::SeqCst);
            Ok(tool_call_message_with_id(
                &format!("call_{request}"),
                r#"{"command":"pwd"}"#,
            ))
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

    #[tokio::test]
    async fn continues_after_a_tool_result_until_final_text() {
        let provider = LoopProvider {
            requests: AtomicUsize::new(0),
        };
        let executor = FakeExecutor;
        let mut messages = initial_messages(String::from("show the current directory"));
        let tools = default_tools();

        let response = run_agent(&provider, &executor, &mut messages, &tools, 10)
            .await
            .expect("agent should finish");

        assert_eq!(response, "You are in /home/test");
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[3].role, "tool");
        assert_eq!(messages[4].role, "assistant");
        assert_eq!(
            messages[4].content.as_deref(),
            Some("You are in /home/test")
        );
    }

    #[tokio::test]
    async fn stops_when_the_turn_limit_is_reached() {
        let provider = AlwaysToolProvider {
            requests: AtomicUsize::new(0),
        };
        let executor = FakeExecutor;
        let mut messages = initial_messages(String::from("keep checking"));
        let tools = default_tools();

        let error = run_agent(&provider, &executor, &mut messages, &tools, 2)
            .await
            .expect_err("agent should stop at its turn limit");

        assert_eq!(error, "Agent reached maximum turn limit of 2");
        assert_eq!(provider.requests.load(Ordering::SeqCst), 2);
        assert_eq!(messages.len(), 6);
    }
}
