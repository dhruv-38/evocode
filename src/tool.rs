use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
pub(crate) struct ToolDefinition {
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) function: FunctionDefinition,
}

#[derive(Debug, Serialize)]
pub(crate) struct FunctionDefinition {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: Value,
}

pub(crate) fn default_tools() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        kind: String::from("function"),
        function: FunctionDefinition {
            name: String::from("bash"),
            description: String::from("Execute a shell command in the current working directory"),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        },
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_tool_has_expected_json_schema() {
        let tools = default_tools();
        let value = serde_json::to_value(&tools[0]).expect("tool should serialize");

        assert_eq!(value["type"], "function");
        assert_eq!(value["function"]["name"], "bash");
        assert_eq!(value["function"]["parameters"]["type"], "object");
        assert_eq!(
            value["function"]["parameters"]["properties"]["command"]["type"],
            "string"
        );
        assert_eq!(value["function"]["parameters"]["required"][0], "command");
        assert_eq!(
            value["function"]["parameters"]["additionalProperties"],
            false
        );
    }
}
