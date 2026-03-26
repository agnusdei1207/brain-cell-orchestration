#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBootstrap {
    profile: String,
}

impl SessionBootstrap {
    pub fn new(profile: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
        }
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }
}

