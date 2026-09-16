use std::env;

mod message;
mod provider;

use message::initial_messages;
use provider::{GroqProvider, ModelProvider};

#[tokio::main]
async fn main() {
    match env::var("GROQ_API_KEY") {
        Ok(key) => {
            let provider = GroqProvider::new(key);
            let input = env::args().collect::<Vec<String>>();

            let prompt = match input.get(1) {
                Some(arg) => arg.to_string(),
                None => {
                    println!("No input provided. Please provide a prompt.");
                    return;
                }
            };

            let mut messages = initial_messages(prompt);

            match provider.send(&messages).await {
                Ok(response) => {
                    println!("Response: {}", response.content);
                    messages.push(response);
                }
                Err(error) => println!("Error: {}", error),
            }
        }
        Err(_) => println!("Error: API Key is not set or invalid!"),
    }
}
