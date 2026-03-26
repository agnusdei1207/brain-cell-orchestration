#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiBlueprint {
    profile_name: &'static str,
}

impl TuiBlueprint {
    pub fn claude_code_inspired() -> Self {
        Self {
            profile_name: "claude-code-inspired-dense-terminal",
        }
    }

    pub fn profile_name(&self) -> &'static str {
        self.profile_name
    }
}

