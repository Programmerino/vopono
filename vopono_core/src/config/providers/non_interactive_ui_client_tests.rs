#[cfg(test)]
mod tests {
    use crate::config::providers::non_interactive_ui_client::NonInteractiveUiClient;
    use crate::config::providers::ui::{
        BoolChoice, ConfigurationChoice, Input, InputNumericu16, Password, UiClient,
    };
    use anyhow::Result;
    use std::fmt::Display;
    use strum::IntoEnumIterator;
    use strum_macros::EnumIter;

    // Mock ConfigurationChoice enum for testing
    #[derive(Debug, EnumIter)]
    enum MockConfigChoice {
        OptionA,
        OptionB,
        OptionC,
    }

    impl Display for MockConfigChoice {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self)
        }
    }

    impl ConfigurationChoice for MockConfigChoice {
        fn prompt(&self) -> String {
            "Test prompt".to_string()
        }
        fn all_names(&self) -> Vec<String> {
            Self::iter().map(|x| format!("{x}")).collect()
        }
        fn all_descriptions(&self) -> Option<Vec<String>> {
            None
        }
        fn description(&self) -> Option<String> {
            None
        }
    }
    impl Default for MockConfigChoice {
        fn default() -> Self {
            Self::OptionA
        }
    }

    #[test]
    fn test_get_configuration_choice() {
        let client = NonInteractiveUiClient;
        let choice = MockConfigChoice::default();
        assert_eq!(client.get_configuration_choice(&choice).unwrap(), 0);
    }

    #[test]
    fn test_get_bool_choice_true() {
        let client = NonInteractiveUiClient;
        let bool_choice = BoolChoice {
            prompt: "Test bool".to_string(),
            default: true,
        };
        assert_eq!(client.get_bool_choice(bool_choice).unwrap(), true);
    }

    #[test]
    fn test_get_bool_choice_false() {
        let client = NonInteractiveUiClient;
        let bool_choice = BoolChoice {
            prompt: "Test bool".to_string(),
            default: false,
        };
        assert_eq!(client.get_bool_choice(bool_choice).unwrap(), false);
    }

    #[test]
    fn test_get_input_returns_error() {
        let client = NonInteractiveUiClient;
        let input = Input {
            prompt: "Test input".to_string(),
            validator: None,
        };
        assert!(client.get_input(input).is_err());
    }

    #[test]
    fn test_get_input_numeric_u16_with_default() {
        let client = NonInteractiveUiClient;
        let input_numeric = InputNumericu16 {
            prompt: "Test numeric input".to_string(),
            validator: None,
            default: Some(123),
        };
        assert_eq!(client.get_input_numeric_u16(input_numeric).unwrap(), 123);
    }

    #[test]
    fn test_get_input_numeric_u16_without_default_returns_error() {
        let client = NonInteractiveUiClient;
        let input_numeric = InputNumericu16 {
            prompt: "Test numeric input".to_string(),
            validator: None,
            default: None,
        };
        assert!(client.get_input_numeric_u16(input_numeric).is_err());
    }

    #[test]
    fn test_get_password_returns_error() {
        let client = NonInteractiveUiClient;
        let password = Password {
            prompt: "Test password".to_string(),
            confirm: false,
        };
        assert!(client.get_password(password).is_err());
    }

    #[test]
    fn test_is_interactive_returns_false() {
        let client = NonInteractiveUiClient;
        assert_eq!(client.is_interactive(), false);
    }
}
