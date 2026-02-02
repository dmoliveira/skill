mod assistant;
mod cli;
mod config;
mod paths;

use anyhow::{anyhow, Result};
use clap::Parser;
use cli::{Cli, Command};
use config::Config;
use paths::AppPaths;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = AppPaths::new()?;
    let mut config = Config::load(&paths)?;

    match cli.command {
        Command::Paths(cmd) => {
            let base_dir = config
                .skills_base_dir
                .as_ref()
                .unwrap_or(&paths.skills_base_dir);
            println!("Config dir: {}", paths.config_dir.display());
            println!("Config file: {}", paths.config_file.display());
            println!("Skills base dir: {}", base_dir.display());

            if let Some(assistant) = cmd.assistant.selected() {
                println!(
                    "Skills root ({assistant}): {}",
                    config.skills_root_for(&paths, assistant).display()
                );
            } else {
                for assistant in [
                    assistant::Assistant::Codex,
                    assistant::Assistant::ClaudeCode,
                    assistant::Assistant::OpenCode,
                ] {
                    println!(
                        "Skills root ({assistant}): {}",
                        config.skills_root_for(&paths, assistant).display()
                    );
                }
            }
            Ok(())
        }
        Command::Default(cmd) => {
            config.default_assistant = Some(cmd.assistant);
            config.save(&paths)?;
            println!("Default assistant set to {}", cmd.assistant);
            Ok(())
        }
        Command::Add(_) => Err(anyhow!("add is not implemented yet")),
        Command::Remove(_) => Err(anyhow!("remove is not implemented yet")),
        Command::List(_) => Err(anyhow!("list is not implemented yet")),
        Command::Show(_) => Err(anyhow!("show is not implemented yet")),
        Command::Stats(_) => Err(anyhow!("stats is not implemented yet")),
        Command::Search(_) => Err(anyhow!("search is not implemented yet")),
        Command::Scan(_) => Err(anyhow!("scan is not implemented yet")),
        Command::Validate(_) => Err(anyhow!("validate is not implemented yet")),
        Command::MarkUsed(_) => Err(anyhow!("mark-used is not implemented yet")),
    }
}
