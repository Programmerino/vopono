use super::ui::{BoolChoice, ConfigurationChoice, Input, InputNumericu16, Password, UiClient};
use anyhow::{anyhow, Result};

pub struct NonInteractiveUiClient;

impl UiClient for NonInteractiveUiClient {
    fn get_configuration_choice(
        &self,
        _conf_choice: &dyn ConfigurationChoice,
    ) -> Result<usize> {
        // Default to the first choice (index 0) in non-interactive mode.
        // This assumes the first option is a sensible default.
        Ok(0)
    }

    fn get_bool_choice(&self, bool_choice: BoolChoice) -> Result<bool> {
        Ok(bool_choice.default)
    }

    fn get_input(&self, input: Input) -> Result<String> {
        Err(anyhow!(
            "Non-interactive mode: Cannot prompt for input: {}",
            input.prompt
        ))
    }

    fn get_input_numeric_u16(&self, input: InputNumericu16) -> Result<u16> {
        if let Some(default_value) = input.default {
            Ok(default_value)
        } else {
            Err(anyhow!(
                "Non-interactive mode: Cannot prompt for numeric input (and no default provided): {}",
                input.prompt
            ))
        }
    }

    fn get_password(&self, password: Password) -> Result<String> {
        Err(anyhow!(
            "Non-interactive mode: Cannot prompt for password: {}",
            password.prompt
        ))
    }

    fn is_interactive(&self) -> bool {
        false
    }
}
