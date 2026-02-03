use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

const SKILLS_HOME_DIR_NAME: &str = ".skills";
const SKILLS_DATA_DIR_NAME: &str = "data";
const CONFIG_FILE_NAME: &str = "config.yaml";
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
        let skills_home = skills_home_dir()?;
        let config_dir = skills_home.clone();
        let data_dir = skills_home.join(SKILLS_DATA_DIR_NAME);
        let skills_base_dir = default_skills_base_dir()?;
        let config_file = config_dir.join(CONFIG_FILE_NAME);
        let usage_file = skills_home.join(USAGE_FILE_NAME);

        Ok(Self {
            config_dir,
            config_file,
            data_dir,
            usage_file,
            skills_base_dir,
        })
    }
}

pub fn default_skills_base_dir() -> Result<PathBuf> {
    default_data_dir()
}

pub fn default_data_dir() -> Result<PathBuf> {
    let home_dir = skills_home_dir()?;
    Ok(home_dir.join(SKILLS_DATA_DIR_NAME))
}

fn skills_home_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("missing home directory"))?;
    Ok(home_dir.join(SKILLS_HOME_DIR_NAME))
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .map_err(|err| anyhow!("failed to create directory {}: {err}", path.display()))
}
