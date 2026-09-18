use std::{env, time::Duration};

const DEFAULT_MODEL: &str = "openai/gpt-oss-20b";
const DEFAULT_MAX_TURNS: usize = 10;
const MAX_ALLOWED_TURNS: usize = 100;
const DEFAULT_BASH_TIMEOUT_SECS: u64 = 30;
const MAX_BASH_TIMEOUT_SECS: u64 = 60 * 60;

pub(crate) struct Config {
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) max_turns: usize,
    pub(crate) bash_timeout: Duration,
}

impl Config {
    pub(crate) fn from_env() -> Result<Self, String> {
        Self::from_values(
            optional_env("GROQ_API_KEY")?,
            optional_env("GROQ_MODEL")?,
            optional_env("AGENT_MAX_TURNS")?,
            optional_env("AGENT_BASH_TIMEOUT_SECS")?,
        )
    }

    pub(crate) fn summary(&self) -> String {
        format!(
            "Active configuration:\n  Model: {}\n  Maximum agent turns: {}\n  Bash timeout: {} seconds",
            self.model,
            self.max_turns,
            self.bash_timeout.as_secs()
        )
    }

    fn from_values(
        api_key: Option<String>,
        model: Option<String>,
        max_turns: Option<String>,
        bash_timeout_secs: Option<String>,
    ) -> Result<Self, String> {
        let api_key = non_empty_value("GROQ_API_KEY", api_key)?;
        let model = match model {
            Some(model) => non_empty_value("GROQ_MODEL", Some(model))?,
            None => String::from(DEFAULT_MODEL),
        };
        let max_turns = bounded_number(
            "AGENT_MAX_TURNS",
            max_turns.as_deref(),
            DEFAULT_MAX_TURNS,
            MAX_ALLOWED_TURNS,
        )?;
        let bash_timeout_secs = bounded_number(
            "AGENT_BASH_TIMEOUT_SECS",
            bash_timeout_secs.as_deref(),
            DEFAULT_BASH_TIMEOUT_SECS,
            MAX_BASH_TIMEOUT_SECS,
        )?;

        Ok(Self {
            api_key,
            model,
            max_turns,
            bash_timeout: Duration::from_secs(bash_timeout_secs),
        })
    }
}

fn optional_env(name: &str) -> Result<Option<String>, String> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{name} contains invalid Unicode")),
    }
}

fn non_empty_value(name: &str, value: Option<String>) -> Result<String, String> {
    let value = value.ok_or_else(|| format!("{name} is not set"))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{name} cannot be empty"));
    }

    Ok(String::from(value))
}

fn bounded_number<T>(name: &str, value: Option<&str>, default: T, maximum: T) -> Result<T, String>
where
    T: Copy + Ord + From<u8> + std::str::FromStr + std::fmt::Display,
{
    let Some(value) = value else {
        return Ok(default);
    };
    let parsed = value
        .trim()
        .parse::<T>()
        .map_err(|_| format!("{name} must be a whole number"))?;
    if parsed < T::from(1) || parsed > maximum {
        return Err(format!("{name} must be between 1 and {maximum}"));
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(
        model: Option<&str>,
        max_turns: Option<&str>,
        timeout: Option<&str>,
    ) -> Result<Config, String> {
        Config::from_values(
            Some(String::from("secret-key")),
            model.map(String::from),
            max_turns.map(String::from),
            timeout.map(String::from),
        )
    }

    #[test]
    fn uses_safe_defaults() {
        let config = config_with(None, None, None).expect("defaults should be valid");

        assert_eq!(config.model, DEFAULT_MODEL);
        assert_eq!(config.max_turns, DEFAULT_MAX_TURNS);
        assert_eq!(
            config.bash_timeout,
            Duration::from_secs(DEFAULT_BASH_TIMEOUT_SECS)
        );
    }

    #[test]
    fn accepts_environment_overrides() {
        let config = config_with(Some("custom-model"), Some("25"), Some("90"))
            .expect("overrides should be valid");

        assert_eq!(config.model, "custom-model");
        assert_eq!(config.max_turns, 25);
        assert_eq!(config.bash_timeout, Duration::from_secs(90));
    }

    #[test]
    fn rejects_missing_api_key() {
        let error = Config::from_values(None, None, None, None)
            .err()
            .expect("missing key should fail");

        assert_eq!(error, "GROQ_API_KEY is not set");
    }

    #[test]
    fn rejects_out_of_range_numbers() {
        assert_eq!(
            config_with(None, Some("0"), None)
                .err()
                .expect("zero turns should fail"),
            "AGENT_MAX_TURNS must be between 1 and 100"
        );
        assert_eq!(
            config_with(None, None, Some("3601"))
                .err()
                .expect("long timeout should fail"),
            "AGENT_BASH_TIMEOUT_SECS must be between 1 and 3600"
        );
    }

    #[test]
    fn summary_never_contains_the_api_key() {
        let config = config_with(None, None, None).expect("config should be valid");

        let summary = config.summary();

        assert!(summary.contains(DEFAULT_MODEL));
        assert!(!summary.contains("secret-key"));
    }
}
