use crate::assistant::Assistant;
use crate::cli::{AddCommand, AssistantArgs, ListCommand, RemoveCommand, ShowCommand};
use crate::config::Config;
use crate::paths::{ensure_dir, AppPaths};
use crate::{scan, validation};
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use walkdir::WalkDir;

pub fn cmd_add(cmd: &AddCommand, config: &Config, paths: &AppPaths) -> Result<()> {
    let assistant = resolve_single_assistant(&cmd.assistant, config, "add")?;
    let (skill_dir, _temp_dir) = prepare_source(&cmd.source)?;

    let validation_report = validation::validate_skill_dir(&skill_dir)?;
    if !validation_report.issues.is_empty() {
        for issue in &validation_report.issues {
            println!("{issue}");
        }
    }
    if validation_report.has_errors() {
        return Err(anyhow!("validation failed"));
    }

    let frontmatter = validation::read_frontmatter(&skill_dir)?;
    let scan_report = scan::scan_path(&skill_dir)?;
    if !scan_report.issues.is_empty() {
        for issue in &scan_report.issues {
            println!("{issue}");
        }
    }
    if !scan_report.external.is_empty() {
        for external in &scan_report.external {
            println!("[{}] {}", external.tool, external.output);
        }
    }
    if scan_report.has_errors() {
        return Err(anyhow!("security scan failed"));
    }

    eprintln!(
        "Warning: Skill usage is at your own risk. Verify and trust the source before installing."
    );

    if !cmd.yes && !confirm("Proceed with installation?")? {
        return Err(anyhow!("installation cancelled"));
    }

    let dest_root = config.skills_root_for(paths, assistant);
    ensure_dir(&dest_root)?;
    let dest_dir = dest_root.join(&frontmatter.name);
    if dest_dir.exists() {
        return Err(anyhow!("skill already exists at {}", dest_dir.display()));
    }

    copy_dir_filtered(&skill_dir, &dest_dir)?;
    println!("Installed {} for {}", frontmatter.name, assistant);
    Ok(())
}

pub fn cmd_remove(cmd: &RemoveCommand, config: &Config, paths: &AppPaths) -> Result<()> {
    let assistant = resolve_single_assistant(&cmd.assistant, config, "remove")?;
    let dest_root = config.skills_root_for(paths, assistant);
    let dest_dir = dest_root.join(&cmd.name);
    if !dest_dir.exists() {
        return Err(anyhow!("skill not found at {}", dest_dir.display()));
    }

    if !cmd.yes && !confirm("Remove this skill?")? {
        return Err(anyhow!("remove cancelled"));
    }

    fs::remove_dir_all(&dest_dir)
        .with_context(|| format!("failed to remove skill directory {}", dest_dir.display()))?;
    println!("Removed {} for {}", cmd.name, assistant);
    Ok(())
}

pub fn cmd_list(cmd: &ListCommand, config: &Config, paths: &AppPaths) -> Result<()> {
    let assistants = resolve_list_assistants(&cmd.assistant, config);

    for assistant in &assistants {
        let root = config.skills_root_for(paths, *assistant);
        let mut names = Vec::new();

        if root.exists() {
            for entry in
                fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))?
            {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let skill_dir = entry.path();
                    if skill_dir.join("SKILL.md").exists() {
                        if let Some(name) = skill_dir.file_name().and_then(|n| n.to_str()) {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }

        names.sort();
        if assistants.len() > 1 {
            println!("{assistant}:");
        }

        if names.is_empty() {
            println!("(no skills found)");
        } else {
            for name in names {
                println!("{name}");
            }
        }

        if assistants.len() > 1 {
            println!();
        }
    }

    Ok(())
}

pub fn cmd_show(cmd: &ShowCommand, config: &Config, paths: &AppPaths) -> Result<()> {
    let assistants = resolve_show_assistants(&cmd.assistant, config);
    let mut found = false;

    for assistant in assistants {
        let root = config.skills_root_for(paths, assistant);
        let skill_dir = root.join(&cmd.name);
        if !skill_dir.exists() {
            continue;
        }

        let frontmatter = validation::read_frontmatter(&skill_dir)?;
        println!("{assistant}:");
        println!("Name: {}", frontmatter.name);
        println!("Description: {}", frontmatter.description);
        println!("Path: {}", skill_dir.display());

        if let Some(compatibility) = frontmatter.compatibility {
            println!("Compatibility: {}", compatibility);
        }
        if let Some(license) = frontmatter.license {
            println!("License: {}", license);
        }
        if let Some(allowed_tools) = frontmatter.allowed_tools {
            println!("Allowed tools: {}", allowed_tools);
        }
        println!();
        found = true;
    }

    if !found {
        return Err(anyhow!("skill not found"));
    }

    Ok(())
}

fn resolve_single_assistant(
    args: &AssistantArgs,
    config: &Config,
    command: &str,
) -> Result<Assistant> {
    if let Some(selected) = args.selected() {
        return Ok(selected);
    }

    if let Some(default) = config.default_assistant {
        eprintln!(
            "Warning: using default assistant {default} for {command}. Use --codex/--claudecode/--opencode to override."
        );
        return Ok(default);
    }

    Err(anyhow!(
        "no assistant selected. Set a default with `skill default <assistant>` or pass --codex/--claudecode/--opencode."
    ))
}

fn resolve_list_assistants(args: &AssistantArgs, config: &Config) -> Vec<Assistant> {
    if let Some(selected) = args.selected() {
        return vec![selected];
    }

    if let Some(default) = config.default_assistant {
        eprintln!(
            "Warning: using default assistant {default} for list. Use --codex/--claudecode/--opencode to override."
        );
        return vec![default];
    }

    eprintln!("Warning: no default assistant set. Listing skills for all assistants.");
    vec![Assistant::Codex, Assistant::ClaudeCode, Assistant::OpenCode]
}

fn resolve_show_assistants(args: &AssistantArgs, config: &Config) -> Vec<Assistant> {
    if let Some(selected) = args.selected() {
        return vec![selected];
    }

    if let Some(default) = config.default_assistant {
        eprintln!(
            "Warning: default assistant is set to {default}. Showing skill across all assistants."
        );
    }
    vec![Assistant::Codex, Assistant::ClaudeCode, Assistant::OpenCode]
}

fn prepare_source(source: &str) -> Result<(PathBuf, Option<TempDir>)> {
    let source_path = PathBuf::from(source);
    if source_path.exists() {
        if !source_path.is_dir() {
            return Err(anyhow!("source path is not a directory"));
        }
        return Ok((source_path, None));
    }

    if looks_like_git_source(source) {
        let temp_dir = tempfile::tempdir().context("failed to create temp dir")?;
        let status = Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg(source)
            .arg(temp_dir.path())
            .status()
            .with_context(|| "failed to run git clone")?;

        if !status.success() {
            return Err(anyhow!("git clone failed"));
        }

        return Ok((temp_dir.path().to_path_buf(), Some(temp_dir)));
    }

    Err(anyhow!("source not found: {source}"))
}

fn looks_like_git_source(source: &str) -> bool {
    source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("git@")
        || source.ends_with(".git")
}

fn confirm(prompt: &str) -> Result<bool> {
    let mut input = String::new();
    print!("{} [y/N]: ", prompt);
    io::stdout().flush()?;
    io::stdin().read_line(&mut input)?;
    let response = input.trim().to_ascii_lowercase();
    Ok(matches!(response.as_str(), "y" | "yes"))
}

fn copy_dir_filtered(src: &Path, dest: &Path) -> Result<()> {
    for entry in WalkDir::new(src).follow_links(false) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(src)?;
        if should_skip(rel_path) {
            continue;
        }

        let target = dest.join(rel_path);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)
                .with_context(|| format!("failed to copy {}", entry.path().display()))?;
        }
    }
    Ok(())
}

fn should_skip(rel_path: &Path) -> bool {
    rel_path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git") | Some("target") | Some(".DS_Store")
        )
    })
}
