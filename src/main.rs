use std::{env, time::Duration};

mod agent;
mod bash;
mod message;
mod provider;
mod tool;

use agent::{DEFAULT_MAX_TURNS, run_agent};
use bash::InteractiveBashExecutor;
use message::initial_messages;
use provider::GroqProvider;
use tool::default_tools;

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
            let tools = default_tools();
            let working_directory = match env::current_dir() {
                Ok(path) => path,
                Err(error) => {
                    println!("Error: Could not determine working directory: {error}");
                    return;
                }
            };
            let executor = InteractiveBashExecutor::new(working_directory, Duration::from_secs(30));

            match run_agent(
                &provider,
                &executor,
                &mut messages,
                &tools,
                DEFAULT_MAX_TURNS,
            )
            .await
            {
                Ok(response) => println!("Response: {}", response),
                Err(error) => println!("Error: {}", error),
            }
        }
        Err(_) => println!("Error: API Key is not set or invalid!"),
    }
}
