mod assistant;
mod cli;
mod commands;
mod config;
mod paths;
mod scan;
mod validation;

use anyhow::{anyhow, Result};
use clap::Parser;
use cli::{Cli, Command};
use config::Config;
use paths::AppPaths;
use std::path::Path;

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
        Command::Add(cmd) => commands::cmd_add(&cmd, &config, &paths),
        Command::Remove(cmd) => commands::cmd_remove(&cmd, &config, &paths),
        Command::List(cmd) => commands::cmd_list(&cmd, &config, &paths),
        Command::Show(cmd) => commands::cmd_show(&cmd, &config, &paths),
        Command::Stats(_) => Err(anyhow!("stats is not implemented yet")),
        Command::Search(_) => Err(anyhow!("search is not implemented yet")),
        Command::Scan(cmd) => {
            let report = scan::scan_path(Path::new(&cmd.path))?;
            if report.issues.is_empty() && report.external.is_empty() {
                println!("Scan passed");
                return Ok(());
            }

            for issue in &report.issues {
                println!("{issue}");
            }

            for external in &report.external {
                println!("[{}] {}", external.tool, external.output);
            }

            if report.has_errors() {
                Err(anyhow!("scan found errors"))
            } else {
                Ok(())
            }
        }
        Command::Validate(cmd) => {
            let report = validation::validate_skill_dir(Path::new(&cmd.path))?;
            if report.issues.is_empty() {
                println!("Validation passed");
                return Ok(());
            }

            for issue in &report.issues {
                println!("{issue}");
            }

            if report.has_errors() {
                Err(anyhow!("validation failed"))
            } else {
                Ok(())
            }
        }
        Command::MarkUsed(_) => Err(anyhow!("mark-used is not implemented yet")),
    }
}
