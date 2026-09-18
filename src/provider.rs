use std::{future::Future, time::Duration};

use reqwest::{StatusCode, header::RETRY_AFTER};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::{message::Message, tool::ToolDefinition};

const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_REQUEST_ATTEMPTS: usize = 3;
const RETRY_BASE_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);
const MAX_ERROR_BODY_CHARS: usize = 1_000;

pub(crate) trait ModelProvider {
    async fn send(&self, messages: &[Message], tools: &[ToolDefinition])
    -> Result<Message, String>;
}

pub(crate) struct GroqProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl GroqProvider {
    pub(crate) fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }

    async fn request_once(&self, request: &ModelRequest<'_>) -> AttemptResult {
        let response = self
            .client
            .post(GROQ_ENDPOINT)
            .bearer_auth(&self.api_key)
            .timeout(REQUEST_TIMEOUT)
            .json(request)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                return AttemptResult::Failure {
                    error: format!("Groq request failed: {error}"),
                    retryable: is_retryable_request_error(&error),
                };
            }
        };

        let status = response.status();
        let retry_after = retry_after_delay(response.headers().get(RETRY_AFTER));
        match response.text().await {
            Ok(body) => AttemptResult::Http {
                status,
                body,
                retry_after,
            },
            Err(error) => AttemptResult::Failure {
                error: format!("Could not read Groq response: {error}"),
                retryable: true,
            },
        }
    }
}

#[derive(Serialize)]
struct ModelRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    tools: &'a [ToolDefinition],
}

#[derive(Deserialize)]
struct ModelResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: ErrorDetails,
}

#[derive(Deserialize)]
struct ErrorDetails {
    message: String,
}

enum AttemptResult {
    Http {
        status: StatusCode,
        body: String,
        retry_after: Option<Duration>,
    },
    Failure {
        error: String,
        retryable: bool,
    },
}

impl ModelProvider for GroqProvider {
    async fn send(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Message, String> {
        let request = ModelRequest {
            model: &self.model,
            messages,
            tools,
        };

        execute_with_retry(|| self.request_once(&request), RETRY_BASE_DELAY).await
    }
}

async fn execute_with_retry<F, Fut>(mut request: F, base_delay: Duration) -> Result<Message, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = AttemptResult>,
{
    for attempt in 1..=MAX_REQUEST_ATTEMPTS {
        match request().await {
            AttemptResult::Http {
                status,
                body,
                retry_after: _,
            } if status.is_success() => return parse_response(&body),
            AttemptResult::Http {
                status,
                body,
                retry_after,
            } => {
                let error = format_http_error(status, &body);
                if is_retryable_status(status) && attempt < MAX_REQUEST_ATTEMPTS {
                    sleep(retry_delay(attempt, base_delay, retry_after)).await;
                    continue;
                }
                return Err(attempted_error(error, attempt));
            }
            AttemptResult::Failure {
                retryable: true, ..
            } if attempt < MAX_REQUEST_ATTEMPTS => {
                sleep(retry_delay(attempt, base_delay, None)).await;
                continue;
            }
            AttemptResult::Failure { error, .. } => {
                return Err(attempted_error(error, attempt));
            }
        }
    }

    unreachable!("request loop always returns on its final attempt")
}

fn is_retryable_request_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout()
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn retry_after_delay(value: Option<&reqwest::header::HeaderValue>) -> Option<Duration> {
    value
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .map(|delay| delay.min(MAX_RETRY_DELAY))
}

fn retry_delay(attempt: usize, base_delay: Duration, retry_after: Option<Duration>) -> Duration {
    retry_after.unwrap_or_else(|| base_delay.saturating_mul(2_u32.pow((attempt - 1) as u32)))
}

fn format_http_error(status: StatusCode, body: &str) -> String {
    let message = serde_json::from_str::<ErrorResponse>(body)
        .ok()
        .map(|response| response.error.message)
        .filter(|message| !message.trim().is_empty())
        .map(|message| bounded_error_body(&message))
        .unwrap_or_else(|| bounded_error_body(body));
    if message.is_empty() {
        format!("Groq returned {status}")
    } else {
        format!("Groq returned {status}: {message}")
    }
}

fn bounded_error_body(body: &str) -> String {
    let mut characters = body.trim().chars();
    let mut preview = characters
        .by_ref()
        .take(MAX_ERROR_BODY_CHARS)
        .collect::<String>();
    if characters.next().is_some() {
        preview.push_str("...");
    }
    preview
}

fn attempted_error(error: String, attempts: usize) -> String {
    if attempts == 1 {
        error
    } else {
        format!("{error} after {attempts} attempts")
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
    use std::{collections::VecDeque, future::ready};

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

    #[tokio::test]
    async fn retries_temporary_errors_and_returns_the_next_response() {
        let mut attempts = VecDeque::from([
            AttemptResult::Http {
                status: StatusCode::SERVICE_UNAVAILABLE,
                body: String::from(r#"{"error":{"message":"try again"}}"#),
                retry_after: None,
            },
            AttemptResult::Http {
                status: StatusCode::OK,
                body: String::from(
                    r#"{"choices":[{"message":{"role":"assistant","content":"recovered"}}]}"#,
                ),
                retry_after: None,
            },
        ]);

        let response = execute_with_retry(
            || ready(attempts.pop_front().expect("an attempt should remain")),
            Duration::ZERO,
        )
        .await
        .expect("request should recover");

        assert_eq!(response.content.as_deref(), Some("recovered"));
        assert!(attempts.is_empty());
    }

    #[tokio::test]
    async fn does_not_retry_permanent_api_errors() {
        let mut attempts = VecDeque::from([
            AttemptResult::Http {
                status: StatusCode::UNAUTHORIZED,
                body: String::from(r#"{"error":{"message":"bad API key"}}"#),
                retry_after: None,
            },
            AttemptResult::Http {
                status: StatusCode::OK,
                body: String::from(
                    r#"{"choices":[{"message":{"role":"assistant","content":"unexpected"}}]}"#,
                ),
                retry_after: None,
            },
        ]);

        let error = execute_with_retry(
            || ready(attempts.pop_front().expect("an attempt should remain")),
            Duration::ZERO,
        )
        .await
        .expect_err("request should fail");

        assert_eq!(error, "Groq returned 401 Unauthorized: bad API key");
        assert_eq!(attempts.len(), 1);
    }

    #[tokio::test]
    async fn reports_when_all_retry_attempts_are_exhausted() {
        let mut attempts = VecDeque::from(
            (0..MAX_REQUEST_ATTEMPTS)
                .map(|_| AttemptResult::Http {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    body: String::from(r#"{"error":{"message":"still unavailable"}}"#),
                    retry_after: None,
                })
                .collect::<Vec<_>>(),
        );

        let error = execute_with_retry(
            || ready(attempts.pop_front().expect("an attempt should remain")),
            Duration::ZERO,
        )
        .await
        .expect_err("request should fail after all attempts");

        assert_eq!(
            error,
            "Groq returned 503 Service Unavailable: still unavailable after 3 attempts"
        );
        assert!(attempts.is_empty());
    }

    #[test]
    fn retries_only_temporary_status_codes() {
        for status in [
            StatusCode::REQUEST_TIMEOUT,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::GATEWAY_TIMEOUT,
        ] {
            assert!(is_retryable_status(status));
        }
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn caps_retry_after_and_error_body_sizes() {
        let retry_after = reqwest::header::HeaderValue::from_static("120");
        assert_eq!(retry_after_delay(Some(&retry_after)), Some(MAX_RETRY_DELAY));

        let body = format!(r#"{{"error":{{"message":"{}"}}}}"#, "x".repeat(2_000));
        let error = format_http_error(StatusCode::BAD_REQUEST, &body);
        assert!(error.ends_with("..."));
        assert!(error.len() < 1_100);
    }
}
