use crate::assistant::Assistant;
use crate::paths::{ensure_dir, AppPaths};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UsageStore {
    #[serde(default)]
    pub skills: BTreeMap<String, UsageCounts>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UsageCounts {
    pub total: u64,
    pub codex: u64,
    pub claudecode: u64,
    pub opencode: u64,
}

impl UsageStore {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        if !paths.usage_file.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&paths.usage_file)
            .with_context(|| format!("failed to read {}", paths.usage_file.display()))?;
        let store = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", paths.usage_file.display()))?;
        Ok(store)
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        ensure_dir(&paths.data_dir)?;
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&paths.usage_file, contents)
            .with_context(|| format!("failed to write {}", paths.usage_file.display()))?;
        Ok(())
    }

    pub fn increment(&mut self, assistant: Assistant, skill: &str) {
        let entry = self.skills.entry(skill.to_string()).or_default();
        entry.total += 1;
        match assistant {
            Assistant::Codex => entry.codex += 1,
            Assistant::ClaudeCode => entry.claudecode += 1,
            Assistant::OpenCode => entry.opencode += 1,
        }
    }

    pub fn count_for(&self, assistant: Assistant, skill: &str) -> u64 {
        self.skills
            .get(skill)
            .map(|entry| match assistant {
                Assistant::Codex => entry.codex,
                Assistant::ClaudeCode => entry.claudecode,
                Assistant::OpenCode => entry.opencode,
            })
            .unwrap_or(0)
    }
}
