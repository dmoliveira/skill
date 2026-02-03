use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

const SKILLS_HOME_DIR_NAME: &str = ".skills";
const SKILLS_DIR_NAME: &str = "AgentSkills";
const CONFIG_FILE_NAME: &str = "config.yaml";
const DATA_DIR_NAME: &str = "skillslash";
const USAGE_FILE_NAME: &str = "usage.json";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub usage_file: PathBuf,
    pub skills_base_dir: PathBuf,
}

impl AppPaths {
    pub fn new() -> Result<Self> {
        let config_dir = default_config_dir()?;
        let data_dir = default_data_dir()?;
        let skills_base_dir = default_skills_base_dir()?;
        let config_file = config_dir.join(CONFIG_FILE_NAME);
        let usage_file = data_dir.join(USAGE_FILE_NAME);

        Ok(Self {
            config_dir,
            config_file,
            data_dir,
            usage_file,
            skills_base_dir,
        })
    }
}

pub fn default_config_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("missing home directory"))?;
    Ok(home_dir.join(SKILLS_HOME_DIR_NAME))
}

pub fn default_skills_base_dir() -> Result<PathBuf> {
    if cfg!(target_os = "linux") {
        let config_base = dirs::config_dir().ok_or_else(|| anyhow!("missing config directory"))?;
        return Ok(config_base.join(SKILLS_DIR_NAME));
    }

    let data_base = dirs::data_dir().ok_or_else(|| anyhow!("missing data directory"))?;
    Ok(data_base.join(SKILLS_DIR_NAME))
}

pub fn default_data_dir() -> Result<PathBuf> {
    let data_base = dirs::data_dir().ok_or_else(|| anyhow!("missing data directory"))?;
    Ok(data_base.join(DATA_DIR_NAME))
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .map_err(|err| anyhow!("failed to create directory {}: {err}", path.display()))
}
