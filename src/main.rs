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

fn main() {
    match env::var("OPENAI_API_KEY") {
        Ok(_key) => {
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
                model: String::from("gpt-3.5-turbo"),
                messages: vec![message],
            };

            let json = serde_json::to_string_pretty(&request);

            match json {
                Ok(json) => println!("{json}"),
                Err(error) => println!("Could not create JSON: {error}"),
            };
        }
        Err(_) => println!("Error: API Key is not set or invalid!"),
    }
}
