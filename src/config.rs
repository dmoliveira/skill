use crate::assistant::Assistant;
use crate::paths::{ensure_dir, AppPaths};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_assistant: Option<Assistant>,
    #[serde(default)]
    pub skills_base_dir: Option<PathBuf>,
    #[serde(default)]
    pub skills_roots: SkillsRoots,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SkillsRoots {
    #[serde(default)]
    pub codex: Option<PathBuf>,
    #[serde(default)]
    pub claudecode: Option<PathBuf>,
    #[serde(default)]
    pub opencode: Option<PathBuf>,
}

impl Config {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        if !paths.config_file.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&paths.config_file).with_context(|| {
            format!("failed to read config file {}", paths.config_file.display())
        })?;
        let config = serde_yaml::from_str(&contents).with_context(|| {
            format!(
                "failed to parse config file {}",
                paths.config_file.display()
            )
        })?;
        Ok(config)
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        ensure_dir(&paths.config_dir)?;
        let contents = serde_yaml::to_string(self)?;
        fs::write(&paths.config_file, contents).with_context(|| {
            format!(
                "failed to write config file {}",
                paths.config_file.display()
            )
        })?;
        Ok(())
    }

    pub fn skills_root_for(&self, paths: &AppPaths, assistant: Assistant) -> PathBuf {
        let override_root = match assistant {
            Assistant::Codex => self.skills_roots.codex.as_ref(),
            Assistant::ClaudeCode => self.skills_roots.claudecode.as_ref(),
            Assistant::OpenCode => self.skills_roots.opencode.as_ref(),
        };

        if let Some(root) = override_root {
            return root.clone();
        }

        let base_dir = self
            .skills_base_dir
            .as_ref()
            .unwrap_or(&paths.skills_base_dir);
        base_dir.join(assistant.as_str())
    }
}
