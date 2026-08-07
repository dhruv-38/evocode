use std::env;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}
fn main() {
    match env::var("OPENAI_API_KEY") {
        Ok(_key) => {
            let prompt: Vec<String> = env::args().collect();

            let message = Message {
                role: String::from("user"),
                content: prompt
                    .get(1)
                    .unwrap_or(&String::from("No input provided"))
                    .to_string(),
            };
            let json = serde_json::to_string(&message);

            match json {
                Ok(json) => println!("{json}"),
                Err(error) => println!("Could not create JSON: {error}"),
            };
        }
        Err(_) => println!("Error: API Key is not set or invalid!"),
    }
}
