use crate::{message::Message, provider::ModelProvider};

pub(crate) async fn run_turn<P: ModelProvider>(
    provider: &P,
    messages: &mut Vec<Message>,
) -> Result<String, String> {
    let response = provider.send(messages).await?;
    let content = response
        .content
        .clone()
        .ok_or_else(|| String::from("The model response did not contain text"))?;
    messages.push(response);

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::initial_messages;

    struct FakeProvider;

    struct FailingProvider;

    impl ModelProvider for FakeProvider {
        async fn send(&self, messages: &[Message]) -> Result<Message, String> {
            assert_eq!(messages.len(), 2);

            Ok(Message::text("assistant", String::from("fake response")))
        }
    }

    impl ModelProvider for FailingProvider {
        async fn send(&self, _messages: &[Message]) -> Result<Message, String> {
            Err(String::from("provider failed"))
        }
    }

    #[tokio::test]
    async fn appends_assistant_response_to_history() {
        let provider = FakeProvider;
        let mut messages = initial_messages(String::from("test prompt"));

        let response = run_turn(&provider, &mut messages)
            .await
            .expect("turn should succeed");

        assert_eq!(response, "fake response");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[2].content.as_deref(), Some("fake response"));
    }

    #[tokio::test]
    async fn leaves_history_unchanged_when_provider_fails() {
        let provider = FailingProvider;
        let mut messages = initial_messages(String::from("test prompt"));

        let error = run_turn(&provider, &mut messages)
            .await
            .expect_err("turn should fail");

        assert_eq!(error, "provider failed");
        assert_eq!(messages.len(), 2);
    }
}
