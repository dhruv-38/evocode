use std::{env, time::Duration};

mod agent;
mod bash;
mod message;
mod provider;
mod tool;

use agent::{TurnOutcome, run_turn};
use bash::{execute_bash, request_approval};
use message::{Message, initial_messages};
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
                    let working_directory = match env::current_dir() {
                        Ok(path) => path,
                        Err(error) => {
                            println!("Error: Could not determine working directory: {error}");
                            return;
                        }
                    };

                    for tool_call in tool_calls {
                        println!("{}: {}", tool_call.tool_call_id, tool_call.command);
                        let tool_result = match request_approval() {
                            Ok(true) => match execute_bash(
                                &tool_call,
                                &working_directory,
                                Duration::from_secs(30),
                            )
                            .await
                            {
                                Ok(output) => {
                                    println!("Tool result for {}", output.tool_call_id);
                                    println!("Exit code: {:?}", output.exit_code);
                                    if !output.stdout.is_empty() {
                                        println!("stdout:\n{}", output.stdout);
                                    }
                                    if !output.stderr.is_empty() {
                                        println!("stderr:\n{}", output.stderr);
                                    }
                                    output.into_tool_result()
                                }
                                Err(error) => {
                                    println!("Tool error: {error}");
                                    Message::tool_result(
                                        tool_call.tool_call_id,
                                        format!("Tool error: {error}"),
                                    )
                                }
                            },
                            Ok(false) => {
                                println!("Command denied");
                                Message::tool_result(
                                    tool_call.tool_call_id,
                                    String::from("Command denied by user"),
                                )
                            }
                            Err(error) => {
                                println!("Approval error: {error}");
                                Message::tool_result(
                                    tool_call.tool_call_id,
                                    format!("Could not request command approval: {error}"),
                                )
                            }
                        };
                        messages.push(tool_result);
                    }
                }
                Err(error) => println!("Error: {}", error),
            }
        }
        Err(_) => println!("Error: API Key is not set or invalid!"),
    }
}
