use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assistant {
    Codex,
    ClaudeCode,
    OpenCode,
}

impl Assistant {
    pub fn as_str(self) -> &'static str {
        match self {
            Assistant::Codex => "codex",
            Assistant::ClaudeCode => "claudecode",
            Assistant::OpenCode => "opencode",
        }
    }
}

impl fmt::Display for Assistant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Assistant {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "codex" => Ok(Assistant::Codex),
            "claudecode" | "claude-code" | "claude_code" => Ok(Assistant::ClaudeCode),
            "opencode" | "open-code" | "open_code" => Ok(Assistant::OpenCode),
            _ => Err(format!(
                "unknown assistant '{value}'. Use codex, claudecode, or opencode."
            )),
        }
    }
}
