use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseKind {
    User,
    BackOff,
    NoProgress,
    Verification,
    Infra,
}

impl PauseKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::BackOff => "back_off",
            Self::NoProgress => "no_progress",
            Self::Verification => "verification",
            Self::Infra => "infra",
        }
    }
}

impl std::str::FromStr for PauseKind {
    type Err = String;

    /// Forgiving on purpose. A workflow script outlives the engine's vocabulary:
    /// the first `execute-jev-plan` run died five minutes in with
    /// "unknown pause kind: Preflight" because the script passed a *phase* name
    /// where a kind was expected. Losing a long run to a string is worse than
    /// rounding, so matching is case- and separator-insensitive and anything
    /// unrecognized pauses for a human — the safe reading of "pause".
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace('-', "_");
        Ok(match normalized.as_str() {
            "back_off" | "backoff" => Self::BackOff,
            "no_progress" => Self::NoProgress,
            "verification" | "blocked" => Self::Verification,
            "infra" => Self::Infra,
            "user" | "human" => Self::User,
            _ => Self::User,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum WorkflowOutcome {
    Completed { result: serde_json::Value },
    Paused { kind: PauseKind, message: String },
    BudgetExceeded { message: String },
    Cancelled,
    Failed { error: String },
}

#[cfg(test)]
mod tests {
    use super::PauseKind;

    #[test]
    fn parses_kinds_case_and_separator_insensitively() {
        assert_eq!("user".parse::<PauseKind>().unwrap(), PauseKind::User);
        assert_eq!("BackOff".parse::<PauseKind>().unwrap(), PauseKind::BackOff);
        assert_eq!("back-off".parse::<PauseKind>().unwrap(), PauseKind::BackOff);
        assert_eq!(
            " NO_PROGRESS ".parse::<PauseKind>().unwrap(),
            PauseKind::NoProgress
        );
        assert_eq!(
            "blocked".parse::<PauseKind>().unwrap(),
            PauseKind::Verification
        );
        assert_eq!("infra".parse::<PauseKind>().unwrap(), PauseKind::Infra);
    }

    #[test]
    fn an_unknown_kind_pauses_for_a_human_instead_of_failing_the_run() {
        // The historical failure: a phase name where a kind was expected killed
        // a run five minutes in.
        assert_eq!("Preflight".parse::<PauseKind>().unwrap(), PauseKind::User);
        assert_eq!("whatever".parse::<PauseKind>().unwrap(), PauseKind::User);
    }
}
