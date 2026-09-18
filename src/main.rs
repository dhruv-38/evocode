use std::env;

mod agent;
mod bash;
mod config;
mod message;
mod provider;
mod repl;
mod session;
mod tool;

use agent::run_agent;
use bash::InteractiveBashExecutor;
use config::Config;
use message::{Message, conversation_messages};
use provider::GroqProvider;
use repl::{ReplCommand, ReplInput, classify_input, format_history, help_text, read_input};
use session::{DEFAULT_SESSION_FILE, load_session, save_session};
use tool::default_tools;

#[tokio::main]
async fn main() {
    tokio::select! {
        () = run_app() => {}
        signal = tokio::signal::ctrl_c() => {
            match signal {
                Ok(()) => eprintln!("\nInterrupted."),
                Err(error) => eprintln!("\nCould not listen for Ctrl+C: {error}"),
            }
            std::process::exit(130);
        }
    }
}

async fn run_app() {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            println!("Error: {error}");
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

    let provider = GroqProvider::new(config.api_key.clone(), config.model.clone());
    let mut messages = conversation_messages(&working_directory);
    let executor = InteractiveBashExecutor::new(working_directory.clone(), config.bash_timeout);
    let tools = default_tools();
    let command_line_prompt = env::args().skip(1).collect::<Vec<_>>().join(" ");
    let mut pending_input =
        (!command_line_prompt.trim().is_empty()).then(|| classify_input(&command_line_prompt));

    println!("Terminal agent ready. Type /exit to quit.");

    loop {
        let input = match pending_input.take() {
            Some(input) => input,
            None => match read_input().await {
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
            ReplInput::Command(ReplCommand::Config) => {
                println!("{}", config.summary());
                continue;
            }
            ReplInput::Command(ReplCommand::Save(path)) => {
                let path = path.map_or_else(
                    || working_directory.join(DEFAULT_SESSION_FILE),
                    |path| working_directory.join(path),
                );
                match save_session(&path, &messages) {
                    Ok(()) => println!("Session saved to {}.", path.display()),
                    Err(error) => println!("Error: {error}"),
                }
                continue;
            }
            ReplInput::Command(ReplCommand::Load(path)) => {
                let path = path.map_or_else(
                    || working_directory.join(DEFAULT_SESSION_FILE),
                    |path| working_directory.join(path),
                );
                match load_session(&path) {
                    Ok(mut loaded_messages) => {
                        messages = conversation_messages(&working_directory);
                        messages.append(&mut loaded_messages);
                        println!("Session loaded from {}.", path.display());
                    }
                    Err(error) => println!("Error: {error}"),
                }
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
            config.max_turns,
        )
        .await
        {
            Ok(response) => println!("\nAssistant:\n{response}\n"),
            Err(error) => eprintln!("Error: {error}"),
        }
    }
}
