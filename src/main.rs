use std::env;

use serde::{Deserialize, Serialize};

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

            let message = Message {
                role: String::from("user"),
                content: prompt,
            };

            let request = ModelRequest {
                model: String::from("openai/gpt-oss-20b"),
                messages: vec![message],
            };
        
            let http_request = client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .bearer_auth(&key)
                .json(&request)
                .send()
                .await;

            match http_request {
                Ok(response) => {
                    let status = response.status();
                    match response.text().await {
                        Ok(body) => {
                            if status.is_success() {
                                let parsed = serde_json::from_str::<ModelResponse>(&body);
                                match parsed {
                                    Ok(model_response) => match model_response.choices.first() {
                                        Some(choice) => {
                                            println!("Assistant: {}", choice.message.content);
                                        }
                                        None => {
                                            println!("The model returned no choices");
                                        }
                                    },
                                    Err(error) => {
                                        println!("Could not parse model response: {error}");
                                        println!("Raw body: {body}");
                                    }
                                }
                            } else {
                                println!("Groq returned {status}: {body}");
                            }
                        }
                        Err(error) => {
                            println!("Could not read response body: {error}");
                        }
                    }
                }
                Err(error) => {
                    println!("Request failed: {error}");
                }
            }
        }
        Err(_) => println!("Error: API Key is not set or invalid!"),
    }
}
