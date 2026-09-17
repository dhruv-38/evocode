use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;
use tokio::{process::Command, time::timeout};

use crate::{
    message::{Message, ToolCall},
    tool::BASH_TOOL_NAME,
};

const MAX_STREAM_OUTPUT_BYTES: usize = 8 * 1024;
const OUTPUT_TRUNCATION_MARKER: &str = "\n... output truncated ...\n";

#[derive(Debug)]
pub(crate) struct BashCall {
    pub(crate) tool_call_id: String,
    pub(crate) command: String,
}

#[derive(Debug)]
pub(crate) struct BashOutput {
    pub(crate) tool_call_id: String,
    pub(crate) exit_code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) trait BashExecutor {
    async fn execute(&self, call: &BashCall) -> Message;
}

pub(crate) struct InteractiveBashExecutor {
    working_directory: PathBuf,
    timeout_duration: Duration,
}

impl InteractiveBashExecutor {
    pub(crate) fn new(working_directory: PathBuf, timeout_duration: Duration) -> Self {
        Self {
            working_directory,
            timeout_duration,
        }
    }
}

impl BashExecutor for InteractiveBashExecutor {
    async fn execute(&self, call: &BashCall) -> Message {
        println!("{}: {}", call.tool_call_id, call.command);

        match request_approval() {
            Ok(true) => match execute_bash(call, &self.working_directory, self.timeout_duration)
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
                    Message::tool_result(call.tool_call_id.clone(), format!("Tool error: {error}"))
                }
            },
            Ok(false) => {
                println!("Command denied");
                Message::tool_result(
                    call.tool_call_id.clone(),
                    String::from("Command denied by user"),
                )
            }
            Err(error) => Message::tool_result(
                call.tool_call_id.clone(),
                format!("Could not request command approval: {error}"),
            ),
        }
    }
}

impl BashOutput {
    pub(crate) fn into_tool_result(self) -> Message {
        let exit_code = self.exit_code.map_or_else(
            || String::from("terminated by signal"),
            |code| code.to_string(),
        );
        let mut content = format!("Exit code: {exit_code}");
        if !self.stdout.is_empty() {
            content.push_str("\nstdout:\n");
            content.push_str(&self.stdout);
        }
        if !self.stderr.is_empty() {
            content.push_str("\nstderr:\n");
            content.push_str(&self.stderr);
        }

        Message::tool_result(self.tool_call_id, content)
    }
}

#[derive(Deserialize)]
struct BashArguments {
    command: String,
}

pub(crate) fn parse_bash_call(tool_call: &ToolCall) -> Result<BashCall, String> {
    if tool_call.kind != "function" {
        return Err(format!("Unsupported tool call type: {}", tool_call.kind));
    }
    if tool_call.function.name != BASH_TOOL_NAME {
        return Err(format!("Unknown tool: {}", tool_call.function.name));
    }

    let arguments = serde_json::from_str::<BashArguments>(&tool_call.function.arguments)
        .map_err(|error| format!("Invalid Bash tool arguments: {error}"))?;
    if arguments.command.trim().is_empty() {
        return Err(String::from("Bash command cannot be empty"));
    }

    Ok(BashCall {
        tool_call_id: tool_call.id.clone(),
        command: arguments.command,
    })
}

pub(crate) fn request_approval() -> Result<bool, String> {
    print!("Allow command? [y/N]: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("Could not display approval prompt: {error}"))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("Could not read approval: {error}"))?;

    Ok(is_approved(&input))
}

pub(crate) async fn execute_bash(
    call: &BashCall,
    working_directory: &Path,
    timeout_duration: Duration,
) -> Result<BashOutput, String> {
    let mut command = Command::new("bash");
    command
        .arg("-lc")
        .arg(&call.command)
        .current_dir(working_directory)
        .kill_on_drop(true);

    let output = timeout(timeout_duration, command.output())
        .await
        .map_err(|_| format!("Bash command timed out after {timeout_duration:?}"))?
        .map_err(|error| format!("Could not execute Bash command: {error}"))?;

    Ok(BashOutput {
        tool_call_id: call.tool_call_id.clone(),
        exit_code: output.status.code(),
        stdout: truncate_middle(
            &String::from_utf8_lossy(&output.stdout),
            MAX_STREAM_OUTPUT_BYTES,
        ),
        stderr: truncate_middle(
            &String::from_utf8_lossy(&output.stderr),
            MAX_STREAM_OUTPUT_BYTES,
        ),
    })
}

fn truncate_middle(output: &str, max_bytes: usize) -> String {
    if output.len() <= max_bytes {
        return String::from(output);
    }
    if max_bytes <= OUTPUT_TRUNCATION_MARKER.len() {
        return String::from(&OUTPUT_TRUNCATION_MARKER[..max_bytes]);
    }

    let available_bytes = max_bytes - OUTPUT_TRUNCATION_MARKER.len();
    let head_budget = available_bytes.div_ceil(2);
    let tail_budget = available_bytes / 2;

    let mut head_end = head_budget;
    while !output.is_char_boundary(head_end) {
        head_end -= 1;
    }

    let mut tail_start = output.len() - tail_budget;
    while !output.is_char_boundary(tail_start) {
        tail_start += 1;
    }

    format!(
        "{}{}{}",
        &output[..head_end],
        OUTPUT_TRUNCATION_MARKER,
        &output[tail_start..]
    )
}

fn is_approved(input: &str) -> bool {
    matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::FunctionCall;

    fn tool_call(name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: String::from("call_123"),
            kind: String::from("function"),
            function: FunctionCall {
                name: String::from(name),
                arguments: String::from(arguments),
            },
        }
    }

    #[test]
    fn parses_valid_bash_arguments() {
        let parsed = parse_bash_call(&tool_call("bash", r#"{"command":"pwd"}"#))
            .expect("arguments should parse");

        assert_eq!(parsed.tool_call_id, "call_123");
        assert_eq!(parsed.command, "pwd");
    }

    #[test]
    fn rejects_unknown_tool() {
        let error = parse_bash_call(&tool_call("read_file", r#"{"command":"pwd"}"#))
            .expect_err("unknown tool should fail");

        assert_eq!(error, "Unknown tool: read_file");
    }

    #[test]
    fn rejects_malformed_arguments() {
        let error = parse_bash_call(&tool_call("bash", "not json"))
            .expect_err("invalid arguments should fail");

        assert!(error.starts_with("Invalid Bash tool arguments:"));
    }

    #[test]
    fn rejects_empty_command() {
        let error = parse_bash_call(&tool_call("bash", r#"{"command":"  "}"#))
            .expect_err("empty command should fail");

        assert_eq!(error, "Bash command cannot be empty");
    }

    #[test]
    fn accepts_only_explicit_approval() {
        assert!(is_approved("y\n"));
        assert!(is_approved("YES"));
        assert!(!is_approved(""));
        assert!(!is_approved("no"));
    }

    #[tokio::test]
    async fn captures_successful_command_output() {
        let call = BashCall {
            tool_call_id: String::from("call_success"),
            command: String::from("printf 'hello'"),
        };

        let output = execute_bash(&call, Path::new("."), Duration::from_secs(1))
            .await
            .expect("command should run");

        assert_eq!(output.tool_call_id, "call_success");
        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.stdout, "hello");
        assert!(output.stderr.is_empty());
    }

    #[tokio::test]
    async fn captures_failed_command_output() {
        let call = BashCall {
            tool_call_id: String::from("call_failure"),
            command: String::from("printf 'failed' >&2; exit 7"),
        };

        let output = execute_bash(&call, Path::new("."), Duration::from_secs(1))
            .await
            .expect("command should run");

        assert_eq!(output.exit_code, Some(7));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, "failed");
    }

    #[tokio::test]
    async fn times_out_long_running_command() {
        let call = BashCall {
            tool_call_id: String::from("call_timeout"),
            command: String::from("sleep 1"),
        };

        let error = execute_bash(&call, Path::new("."), Duration::from_millis(20))
            .await
            .expect_err("command should time out");

        assert!(error.starts_with("Bash command timed out after"));
    }

    #[tokio::test]
    async fn limits_large_command_output() {
        let call = BashCall {
            tool_call_id: String::from("call_large_output"),
            command: String::from("printf 'start'; printf '%09000d' 0; printf 'end'"),
        };

        let output = execute_bash(&call, Path::new("."), Duration::from_secs(1))
            .await
            .expect("command should run");

        assert!(output.stdout.len() <= MAX_STREAM_OUTPUT_BYTES);
        assert!(output.stdout.starts_with("start"));
        assert!(output.stdout.ends_with("end"));
        assert!(output.stdout.contains(OUTPUT_TRUNCATION_MARKER));
    }

    #[test]
    fn leaves_small_output_unchanged() {
        assert_eq!(truncate_middle("short output", 64), "short output");
    }

    #[test]
    fn truncates_the_middle_and_preserves_both_ends() {
        let output = format!("start{}end", "x".repeat(100));

        let truncated = truncate_middle(&output, 64);

        assert_eq!(truncated.len(), 64);
        assert!(truncated.starts_with("start"));
        assert!(truncated.ends_with("end"));
        assert!(truncated.contains(OUTPUT_TRUNCATION_MARKER));
    }

    #[test]
    fn truncates_utf8_only_at_character_boundaries() {
        let output = "😀".repeat(100);

        let truncated = truncate_middle(&output, 64);

        assert!(truncated.len() <= 64);
        assert!(truncated.starts_with('😀'));
        assert!(truncated.ends_with('😀'));
        assert!(truncated.contains(OUTPUT_TRUNCATION_MARKER));
    }

    #[test]
    fn converts_output_to_tool_result_message() {
        let message = BashOutput {
            tool_call_id: String::from("call_output"),
            exit_code: Some(7),
            stdout: String::from("partial output"),
            stderr: String::from("failure"),
        }
        .into_tool_result();

        let content = message.content.expect("tool result should contain text");
        assert_eq!(message.role, "tool");
        assert_eq!(message.tool_call_id.as_deref(), Some("call_output"));
        assert!(content.contains("Exit code: 7"));
        assert!(content.contains("stdout:\npartial output"));
        assert!(content.contains("stderr:\nfailure"));
    }
}
