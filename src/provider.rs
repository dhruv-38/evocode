use serde::{Deserialize, Serialize};

use crate::message::Message;

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

pub(crate) async fn send_request(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[Message],
) -> Result<Message, String> {
    let request = ModelRequest {
        model: "openai/gpt-oss-20b",
        messages,
    };

    let response = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .bearer_auth(api_key)
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
        assert_eq!(message.content, "hello");
    }

    #[test]
    fn rejects_response_without_choices() {
        let error = parse_response(r#"{"choices": []}"#).expect_err("response should fail");

        assert_eq!(error, "The model returned no choices");
    }
}
