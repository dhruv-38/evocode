use std::{env, time::Duration};

mod agent;
mod bash;
mod message;
mod provider;
mod repl;
mod tool;

use agent::{DEFAULT_MAX_TURNS, run_agent};
use bash::InteractiveBashExecutor;
use message::{Message, conversation_messages};
use provider::GroqProvider;
use repl::{ReplInput, read_input};
use tool::default_tools;

#[tokio::main]
async fn main() {
    let api_key = match env::var("GROQ_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("Error: API Key is not set or invalid!");
            return;
        }
    };
    let working_directory = match env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            println!("Error: Could not determine working directory: {error}");
            return;
        }
    };

    let provider = GroqProvider::new(api_key);
    let executor = InteractiveBashExecutor::new(working_directory, Duration::from_secs(30));
    let tools = default_tools();
    let mut messages = conversation_messages();
    let command_line_prompt = env::args().skip(1).collect::<Vec<_>>().join(" ");
    let mut pending_prompt =
        (!command_line_prompt.trim().is_empty()).then_some(command_line_prompt);

    println!("Terminal agent ready. Type /exit to quit.");

    loop {
        let prompt = match pending_prompt.take() {
            Some(prompt) => prompt,
            None => match read_input() {
                Ok(ReplInput::Prompt(prompt)) => prompt,
                Ok(ReplInput::Empty) => continue,
                Ok(ReplInput::Exit) => break,
                Err(error) => {
                    println!("Error: {error}");
                    break;
                }
            },
        };

        messages.push(Message::text("user", prompt));
        match run_agent(
            &provider,
            &executor,
            &mut messages,
            &tools,
            DEFAULT_MAX_TURNS,
        )
        .await
        {
            Ok(response) => println!("Response: {response}"),
            Err(error) => println!("Error: {error}"),
        }
    }
}
