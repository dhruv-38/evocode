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
use repl::{ReplCommand, ReplInput, classify_input, format_history, help_text, read_input};
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
    let mut messages = conversation_messages(&working_directory);
    let executor = InteractiveBashExecutor::new(working_directory.clone(), Duration::from_secs(30));
    let tools = default_tools();
    let command_line_prompt = env::args().skip(1).collect::<Vec<_>>().join(" ");
    let mut pending_input =
        (!command_line_prompt.trim().is_empty()).then(|| classify_input(&command_line_prompt));

    println!("Terminal agent ready. Type /exit to quit.");

    loop {
        let input = match pending_input.take() {
            Some(input) => input,
            None => match read_input() {
                Ok(input) => input,
                Err(error) => {
                    println!("Error: {error}");
                    break;
                }
            },
        };

        let prompt = match input {
            ReplInput::Prompt(prompt) => prompt,
            ReplInput::Command(ReplCommand::Help) => {
                println!("{}", help_text());
                continue;
            }
            ReplInput::Command(ReplCommand::Clear) => {
                messages = conversation_messages(&working_directory);
                println!("Conversation cleared.");
                continue;
            }
            ReplInput::Command(ReplCommand::History) => {
                println!("{}", format_history(&messages));
                continue;
            }
            ReplInput::UnknownCommand(command) => {
                println!("Unknown command: {command}. Type /help for available commands.");
                continue;
            }
            ReplInput::Empty => continue,
            ReplInput::Exit => break,
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
