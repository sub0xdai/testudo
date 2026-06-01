// @anchor infra:cli:commands
// @tags ui

//! TUI command parser — maps slash-command strings to actions.

/// Commands available in the TUI command palette.
#[derive(Debug, Clone, PartialEq)]
pub enum TuiCommand {
    Dashboard,
    Journal,
    Strategies,
    Logs,
    Help,
    Settings,
    Quit,
}

impl TuiCommand {
    /// Parse a slash-command string like "/strategies" or ":q".
    /// Returns None for unrecognized commands.
    pub fn from_input(input: &str) -> Option<Self> {
        match input.trim() {
            "/dashboard" | ":dashboard" => Some(Self::Dashboard),
            "/journal" | ":journal" => Some(Self::Journal),
            "/strategies" | ":strategies" => Some(Self::Strategies),
            "/logs" | ":logs" => Some(Self::Logs),
            "/help" | ":help" => Some(Self::Help),
            "/settings" | ":settings" => Some(Self::Settings),
            "/quit" | "/q" | ":quit" | ":q" => Some(Self::Quit),
            _ => None,
        }
    }

    /// All available commands for autocomplete.
    pub fn all() -> &'static [&'static str] {
        &[
            "/dashboard",
            "/journal",
            "/strategies",
            "/logs",
            "/help",
            "/settings",
            "/quit",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dashboard() {
        assert_eq!(TuiCommand::from_input("/dashboard"), Some(TuiCommand::Dashboard));
        assert_eq!(TuiCommand::from_input(":dashboard"), Some(TuiCommand::Dashboard));
    }

    #[test]
    fn parse_journal() {
        assert_eq!(TuiCommand::from_input("/journal"), Some(TuiCommand::Journal));
        assert_eq!(TuiCommand::from_input(":journal"), Some(TuiCommand::Journal));
    }

    #[test]
    fn parse_strategies() {
        assert_eq!(TuiCommand::from_input("/strategies"), Some(TuiCommand::Strategies));
    }

    #[test]
    fn parse_logs() {
        assert_eq!(TuiCommand::from_input("/logs"), Some(TuiCommand::Logs));
    }

    #[test]
    fn parse_help() {
        assert_eq!(TuiCommand::from_input("/help"), Some(TuiCommand::Help));
    }

    #[test]
    fn parse_settings() {
        assert_eq!(TuiCommand::from_input("/settings"), Some(TuiCommand::Settings));
        assert_eq!(TuiCommand::from_input(":settings"), Some(TuiCommand::Settings));
    }

    #[test]
    fn parse_quit_variants() {
        assert_eq!(TuiCommand::from_input("/quit"), Some(TuiCommand::Quit));
        assert_eq!(TuiCommand::from_input("/q"), Some(TuiCommand::Quit));
        assert_eq!(TuiCommand::from_input(":quit"), Some(TuiCommand::Quit));
        assert_eq!(TuiCommand::from_input(":q"), Some(TuiCommand::Quit));
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert_eq!(TuiCommand::from_input("/xyz"), None);
        assert_eq!(TuiCommand::from_input(":unknown"), None);
        assert_eq!(TuiCommand::from_input(""), None);
        assert_eq!(TuiCommand::from_input("dashboard"), None);
        assert_eq!(TuiCommand::from_input("/"), None);
        assert_eq!(TuiCommand::from_input(":"), None);
    }

    #[test]
    fn parse_case_sensitive() {
        assert_eq!(TuiCommand::from_input("/DASHBOARD"), None);
        assert_eq!(TuiCommand::from_input("/Quit"), None);
    }

    #[test]
    fn all_commands_list_is_complete() {
        let all = TuiCommand::all();
        assert!(all.contains(&"/dashboard"));
        assert!(all.contains(&"/journal"));
        assert!(all.contains(&"/strategies"));
        assert!(all.contains(&"/logs"));
        assert!(all.contains(&"/help"));
        assert!(all.contains(&"/settings"));
        assert!(all.contains(&"/quit"));
        assert_eq!(all.len(), 7);
    }

    #[test]
    fn each_command_parses_from_its_own_name() {
        for cmd_str in TuiCommand::all() {
            let parsed = TuiCommand::from_input(cmd_str);
            assert!(parsed.is_some(), "failed to parse: {}", cmd_str);
        }
    }
}
