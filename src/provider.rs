use serde::{Deserialize, Serialize};

use crate::message::Message;

const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";
const MODEL: &str = "openai/gpt-oss-20b";

pub(crate) trait ModelProvider {
    async fn send(&self, messages: &[Message]) -> Result<Message, String>;
}

pub(crate) struct GroqProvider {
    client: reqwest::Client,
    api_key: String,
}

impl GroqProvider {
    pub(crate) fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }
}

#[derive(Serialize)]
struct ModelRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
}

#[derive(Deserialize)]
struct ModelResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

impl ModelProvider for GroqProvider {
    async fn send(&self, messages: &[Message]) -> Result<Message, String> {
        let request = ModelRequest {
            model: MODEL,
            messages,
        };

        let response = self
            .client
            .post(GROQ_ENDPOINT)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|error| format!("Request failed: {error}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("Could not read response body: {error}"))?;
        if !status.is_success() {
            return Err(format!("Groq returned {status}: {body}"));
        }

        parse_response(&body)
    }
}

fn parse_response(body: &str) -> Result<Message, String> {
    let parsed = serde_json::from_str::<ModelResponse>(body)
        .map_err(|error| format!("Could not parse response body: {error}"))?;
    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| String::from("The model returned no choices"))?;

    Ok(choice.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_assistant_message_from_response() {
        let body = r#"{
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "hello"
                    }
                }
            ]
        }"#;

        let message = parse_response(body).expect("response should parse");

        assert_eq!(message.role, "assistant");
        assert_eq!(message.content.as_deref(), Some("hello"));
        assert!(message.tool_calls.is_empty());
    }

    #[test]
    fn rejects_response_without_choices() {
        let error = parse_response(r#"{"choices": []}"#).expect_err("response should fail");

        assert_eq!(error, "The model returned no choices");
    }

    #[test]
    fn parses_tool_call_from_response() {
        let body = r#"{
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [
                            {
                                "id": "call_123",
                                "type": "function",
                                "function": {
                                    "name": "bash",
                                    "arguments": "{\"command\":\"pwd\"}"
                                }
                            }
                        ]
                    }
                }
            ]
        }"#;

        let message = parse_response(body).expect("response should parse");
        let tool_call = message.tool_calls.first().expect("tool call should exist");

        assert!(message.content.is_none());
        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.kind, "function");
        assert_eq!(tool_call.function.name, "bash");
        assert_eq!(tool_call.function.arguments, r#"{"command":"pwd"}"#);
    }
}
