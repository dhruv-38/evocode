use std::io::{self, Write};

use crate::message::Message;

const HISTORY_PREVIEW_CHARS: usize = 200;
const HISTORY_MAX_ENTRIES: usize = 20;

#[derive(Debug, PartialEq)]
pub(crate) enum ReplCommand {
    Help,
    Clear,
    History,
    Config,
    Save(Option<String>),
    Load(Option<String>),
}

#[derive(Debug, PartialEq)]
pub(crate) enum ReplInput {
    Prompt(String),
    Command(ReplCommand),
    UnknownCommand(String),
    Empty,
    Exit,
}

pub(crate) async fn read_input() -> Result<ReplInput, String> {
    tokio::task::spawn_blocking(read_input_blocking)
        .await
        .map_err(|error| format!("Prompt task failed: {error}"))?
}

fn read_input_blocking() -> Result<ReplInput, String> {
    print!("> ");
    io::stdout()
        .flush()
        .map_err(|error| format!("Could not display prompt: {error}"))?;

    let mut input = String::new();
    let bytes_read = io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("Could not read prompt: {error}"))?;
    if bytes_read == 0 {
        return Ok(ReplInput::Exit);
    }

    Ok(classify_input(&input))
}

pub(crate) fn classify_input(input: &str) -> ReplInput {
    let input = input.trim();
    if input.is_empty() {
        return ReplInput::Empty;
    }
    if ["exit", "quit", "/exit", "/quit"]
        .iter()
        .any(|command| input.eq_ignore_ascii_case(command))
    {
        return ReplInput::Exit;
    }

    let command_end = input.find(char::is_whitespace).unwrap_or(input.len());
    let command = &input[..command_end];
    let argument = input[command_end..].trim();
    let argument = (!argument.is_empty()).then(|| String::from(argument));

    match command.to_ascii_lowercase().as_str() {
        "/help" => return ReplInput::Command(ReplCommand::Help),
        "/clear" => return ReplInput::Command(ReplCommand::Clear),
        "/history" => return ReplInput::Command(ReplCommand::History),
        "/config" => return ReplInput::Command(ReplCommand::Config),
        "/save" => return ReplInput::Command(ReplCommand::Save(argument)),
        "/load" => return ReplInput::Command(ReplCommand::Load(argument)),
        _ => {}
    }
    if input.starts_with('/') {
        return ReplInput::UnknownCommand(String::from(input));
    }

    ReplInput::Prompt(String::from(input))
}

pub(crate) fn help_text() -> &'static str {
    "Commands:\n  /help         Show available commands\n  /clear        Start a new conversation\n  /history      Show this conversation\n  /config       Show active configuration\n  /save [path]  Save this conversation\n  /load [path]  Load a saved conversation\n  /exit         Exit the agent"
}

pub(crate) fn format_history(messages: &[Message]) -> String {
    let message_count = messages
        .iter()
        .filter(|message| message.role != "system")
        .count();
    if message_count == 0 {
        return String::from("No conversation history.");
    }

    let skipped_entries = message_count.saturating_sub(HISTORY_MAX_ENTRIES);
    let entries = messages
        .iter()
        .filter(|message| message.role != "system")
        .skip(skipped_entries)
        .enumerate()
        .map(|(index, message)| {
            let label = match message.role.as_str() {
                "user" => String::from("User"),
                "assistant" => String::from("Assistant"),
                "tool" => message.tool_call_id.as_ref().map_or_else(
                    || String::from("Tool"),
                    |tool_call_id| format!("Tool ({tool_call_id})"),
                ),
                role => String::from(role),
            };

            format!(
                "{}. {label}: {}",
                index + skipped_entries + 1,
                message_preview(message)
            )
        })
        .collect::<Vec<_>>();

    let heading = if skipped_entries == 0 {
        String::from("Conversation history:")
    } else {
        format!("Conversation history (last {HISTORY_MAX_ENTRIES} of {message_count} entries):")
    };
    format!("{heading}\n{}", entries.join("\n"))
}

fn message_preview(message: &Message) -> String {
    let mut parts = Vec::new();
    if let Some(content) = message
        .content
        .as_deref()
        .filter(|content| !content.is_empty())
    {
        parts.push(String::from(content));
    }
    parts.extend(message.tool_calls.iter().map(|tool_call| {
        format!(
            "tool call {} {}",
            tool_call.function.name, tool_call.function.arguments
        )
    }));

    let content = if parts.is_empty() {
        String::from("(empty)")
    } else {
        parts.join(" | ")
    };
    single_line_preview(&content)
}

fn single_line_preview(content: &str) -> String {
    let content = content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', " ↩ ");
    let mut characters = content.chars();
    let mut preview = characters
        .by_ref()
        .take(HISTORY_PREVIEW_CHARS)
        .collect::<String>();
    if characters.next().is_some() {
        preview.push_str("...");
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_text_into_a_prompt() {
        assert_eq!(
            classify_input("  explain this project  \n"),
            ReplInput::Prompt(String::from("explain this project"))
        );
    }

    #[test]
    fn recognizes_exit_commands() {
        for command in ["exit", "QUIT", "/exit", "/QUIT"] {
            assert_eq!(classify_input(command), ReplInput::Exit);
        }
    }

    #[test]
    fn recognizes_conversation_commands() {
        assert_eq!(
            classify_input("/HELP"),
            ReplInput::Command(ReplCommand::Help)
        );
        assert_eq!(
            classify_input("/clear"),
            ReplInput::Command(ReplCommand::Clear)
        );
        assert_eq!(
            classify_input("/history"),
            ReplInput::Command(ReplCommand::History)
        );
        assert_eq!(
            classify_input("/config"),
            ReplInput::Command(ReplCommand::Config)
        );
        assert_eq!(
            classify_input("/save"),
            ReplInput::Command(ReplCommand::Save(None))
        );
        assert_eq!(
            classify_input("/load my session.json"),
            ReplInput::Command(ReplCommand::Load(Some(String::from("my session.json"))))
        );
    }

    #[test]
    fn identifies_unknown_slash_commands() {
        assert_eq!(
            classify_input("/missing"),
            ReplInput::UnknownCommand(String::from("/missing"))
        );
    }

    #[test]
    fn ignores_empty_input() {
        assert_eq!(classify_input("  \n"), ReplInput::Empty);
    }

    #[test]
    fn formats_bounded_conversation_history() {
        let messages = vec![
            Message::text("system", String::from("instructions")),
            Message::text("user", String::from("first line\nsecond line")),
            Message::text("assistant", "x".repeat(HISTORY_PREVIEW_CHARS + 10)),
            Message::tool_result(String::from("call_123"), String::from("Exit code: 0")),
        ];

        let history = format_history(&messages);

        assert!(!history.contains("instructions"));
        assert!(history.contains("1. User: first line ↩ second line"));
        assert!(history.contains("2. Assistant:"));
        assert!(history.contains("xxx..."));
        assert!(history.contains("3. Tool (call_123): Exit code: 0"));
    }

    #[test]
    fn reports_empty_conversation_history() {
        let messages = vec![Message::text("system", String::from("instructions"))];

        assert_eq!(format_history(&messages), "No conversation history.");
    }

    #[test]
    fn history_only_shows_the_most_recent_entries() {
        let mut messages = vec![Message::text("system", String::from("instructions"))];
        messages.extend((0..25).map(|index| Message::text("user", format!("message {index}"))));

        let history = format_history(&messages);

        assert!(history.contains("last 20 of 25 entries"));
        assert!(!history.contains("1. User: message 0"));
        assert!(history.contains("6. User: message 5"));
        assert!(history.contains("25. User: message 24"));
    }
}
