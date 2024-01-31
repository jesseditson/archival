use std::error::Error;

#[derive(Debug)]
pub struct ArchivalError {
    message: String,
}
impl ArchivalError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_owned(),
        }
    }
}
impl std::fmt::Display for ArchivalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Archival Error: {}", self.message)
    }
}
impl Error for ArchivalError {
    fn description(&self) -> &str {
        &self.message
    }
}
