use std::env;

mod agent;
mod bash;
mod message;
mod provider;
mod tool;

use agent::{TurnOutcome, run_turn};
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

            match run_turn(&provider, &mut messages, &tools).await {
                Ok(TurnOutcome::FinalText(response)) => println!("Response: {}", response),
                Ok(TurnOutcome::ToolCalls(tool_calls)) => {
                    println!("The model requested {} tool call(s)", tool_calls.len());
                    for tool_call in tool_calls {
                        println!("{}: {}", tool_call.tool_call_id, tool_call.command);
                    }
                }
                Err(error) => println!("Error: {}", error),
            }
        }
        Err(_) => println!("Error: API Key is not set or invalid!"),
    }
}
