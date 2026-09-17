use std::io::{self, Write};

#[derive(Debug, PartialEq)]
pub(crate) enum ReplInput {
    Prompt(String),
    Empty,
    Exit,
}

pub(crate) fn read_input() -> Result<ReplInput, String> {
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

fn classify_input(input: &str) -> ReplInput {
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

    ReplInput::Prompt(String::from(input))
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
    fn ignores_empty_input() {
        assert_eq!(classify_input("  \n"), ReplInput::Empty);
    }
}
