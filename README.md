# Evocode

Evocode is an interactive AI coding agent that can inspect and modify a local project by requesting Bash commands. It uses Groq's OpenAI-compatible chat completions API and asks for approval before every command.

This repository is a learning-focused V1 inspired by terminal agent harnesses such as Pi. The implementation is intentionally small enough to understand while still including a real agent loop, tool execution, conversation history, sessions, retries, configuration, and safety limits.

## Features

- Multi-turn interactive conversations with in-memory history
- A bounded model → tool → result agent loop
- Bash tool calls with explicit `y`/`yes` approval
- Command timeout and captured exit code, stdout, and stderr
- Output truncation that preserves the beginning and end
- Versioned conversation save/load files
- Configurable model, turn limit, and Bash timeout
- Retries for temporary Groq and network failures
- Ctrl+C handling and local conversation commands
- Unit and black-box CLI test suites

## Requirements

- A recent stable Rust toolchain with Cargo (the crate uses Rust 2024 edition)
- `bash` available on `PATH`
- A [Groq API key](https://console.groq.com/keys)
- A Groq model that supports tool calling

## Install

After cloning the repository, install the binary from the project directory:

```bash
cd evocode
cargo install --path .
```

For development, run it without installing:

```bash
cargo run
```

## Configure

Set the required API key in your shell. Do not commit the key to source control.

```bash
export GROQ_API_KEY="your-groq-api-key"
```

Optional settings:

| Environment variable | Default | Allowed values | Purpose |
|---|---:|---:|---|
| `GROQ_MODEL` | `openai/gpt-oss-20b` | Non-empty model name | Groq model used for requests |
| `AGENT_MAX_TURNS` | `10` | `1`–`100` | Maximum model turns for one user prompt |
| `AGENT_BASH_TIMEOUT_SECS` | `30` | `1`–`3600` | Maximum duration of each Bash command |

Use `/config` inside the agent to inspect the active non-secret settings. The API key is never included in that output.

## Use

Start an interactive session in the project you want the agent to work on:

```bash
cd /path/to/your/project
evocode
```

Then enter a request:

```text
> explain how this project is structured
```

You can also provide the first prompt as command-line arguments. The program remains interactive after answering it:

```bash
evocode "find and explain the failing tests"
```

When the model requests Bash, review the exact command before approving it:

```text
Bash requested:
$ cargo test
Allow command? [y/N]: y
```

Only `y` or `yes`, ignoring case, approves a command. Any other response denies it.

## Conversation commands

| Command | Description |
|---|---|
| `/help` | Show the available commands |
| `/clear` | Clear conversation history and rebuild the system prompt |
| `/history` | Show up to the 20 most recent history entries |
| `/config` | Show active non-secret configuration |
| `/save [path]` | Save the current conversation |
| `/load [path]` | Load a saved conversation |
| `/exit` | Exit the agent |

`exit`, `quit`, and `/quit` also exit. Empty input is ignored.

Without a path, `/save` and `/load` use `.evocode-session.json` in the working directory. Session files:

- do not store the configured API key automatically;
- omit the system prompt and regenerate it for the current directory when loaded;
- use a versioned JSON format;
- are limited to 10 MiB.

They can contain prompts, model responses, commands, and tool output, so treat them as potentially sensitive. The default session filename is ignored by this repository. Add it to other projects' ignore files if you save sessions there.

## Safety and execution behavior

This agent is **not a security sandbox**. Approved commands run with your user's permissions. Read every command before approving it, especially commands that install software, change Git history, or delete files.

V1 applies these boundaries:

- Every Bash command requires approval.
- Each command starts in the directory where the agent was launched.
- A `cd` affects only its individual Bash call and does not persist.
- Interactive Bash programs are not supported.
- The Bash process is terminated after the configured timeout.
- Stdout and stderr are each limited to 8 KiB before reaching the model. Long output keeps its beginning and end with a truncation marker.
- One user prompt is limited to the configured number of model turns.
- Groq requests time out after 60 seconds and retry temporary failures up to three attempts.
- Ctrl+C stops the process with exit code 130.

## Architecture

The project separates the harness into focused modules:

```text
src/
├── main.rs      CLI lifecycle and conversation loop
├── agent.rs     bounded model/tool agent loop
├── provider.rs  Groq HTTP requests, response parsing, and retries
├── bash.rs      Bash validation, approval, execution, and output limits
├── message.rs   chat and tool-call message types
├── tool.rs      Bash tool schema sent to the model
├── repl.rs      local commands and history formatting
├── session.rs   versioned session persistence
└── config.rs    environment configuration and validation
```

The central loop is:

```text
user prompt
    ↓
Groq model response
    ├── final text → display to user
    └── Bash call → approval → execution → tool result ─┐
                                                        └─ back to model
```

## Development

Run the project checks:

```bash
cargo fmt -- --check
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The test suite includes fake-provider agent-loop tests, Bash execution tests, retry-policy tests, session round trips, and black-box tests that launch the compiled CLI.

## V1 limitations

- Groq is the only provider.
- Bash is the only tool; there are no dedicated read, write, or edit tools.
- Model output is displayed after the full response rather than streamed token by token.
- Conversation context is not summarized or compacted automatically.
- Sessions must be saved and loaded explicitly.
- Shell environment changes do not persist between tool calls.
- Windows requires an environment that provides `bash`.

Natural post-V1 extensions include response streaming, dedicated filesystem tools, context compaction, multiple providers, richer terminal rendering, and project-specific instruction files.
