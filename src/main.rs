use std::env;

use serde::{Deserialize, Serialize};

const SYSTEM_PROMPT: &str = "You are a helpful coding assistant.";

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct ModelRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct ModelResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[tokio::main]
async fn main() {
    match env::var("GROQ_API_KEY") {
        Ok(key) => {
            let client = reqwest::Client::new();
            let input = env::args().collect::<Vec<String>>();

            let prompt = match input.get(1) {
                Some(arg) => arg.to_string(),
                None => {
                    println!("No input provided. Please provide a prompt.");
                    return;
                }
            };

            let messages = initial_messages(prompt);

            match send_request(&client, &key, messages).await {
                Ok(response) => println!("Response: {}", response),
                Err(error) => println!("Error: {}", error),
            }
        }
        Err(_) => println!("Error: API Key is not set or invalid!"),
    }
}

fn initial_messages(prompt: String) -> Vec<Message> {
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

async fn send_request(
    client: &reqwest::Client,
    api_key: &str,
    messages: Vec<Message>,
) -> Result<String, String> {
    let request = ModelRequest {
        model: String::from("openai/gpt-oss-20b"),
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
    let parsed = serde_json::from_str::<ModelResponse>(&body)
        .map_err(|error| format!("Could not parse response body: {error}"))?;
    let choice = parsed
        .choices
        .first()
        .ok_or_else(|| String::from("The model returned no choices"))?;
    Ok(choice.message.content.clone())
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
