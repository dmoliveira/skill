use crate::assistant::Assistant;
use crate::cli::{
    AddCommand, AssistantArgs, ListCommand, MarkUsedCommand, RemoveCommand, SearchCommand,
    ShowCommand, StatsCommand,
};
use crate::config::Config;
use crate::paths::{ensure_dir, AppPaths};
use crate::usage::UsageStore;
use crate::{scan, validation};
use anyhow::{anyhow, Context, Result};
use bytesize::ByteSize;
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

pub fn cmd_search(cmd: &SearchCommand, config: &Config, paths: &AppPaths) -> Result<()> {
    let assistants = resolve_list_assistants(&cmd.assistant, config);
    let query = cmd.query.to_ascii_lowercase();
    let mut matches = Vec::new();

    for assistant in &assistants {
        let root = config.skills_root_for(paths, *assistant);
        if !root.exists() {
            continue;
        }

        for entry in
            fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))?
        {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let skill_dir = entry.path();
            let skill_md = skill_dir.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            let contents = fs::read_to_string(&skill_md)
                .with_context(|| format!("failed to read {}", skill_md.display()))?;
            let frontmatter = validation::read_frontmatter(&skill_dir)?;
            let haystack = format!(
                "{}\n{}\n{}",
                frontmatter.name, frontmatter.description, contents
            )
            .to_ascii_lowercase();

            if haystack.contains(&query) {
                matches.push((
                    assistant,
                    frontmatter.name,
                    frontmatter.description,
                    skill_dir,
                ));
            }
        }
    }

    if matches.is_empty() {
        println!("No matches found");
        return Ok(());
    }

    for (assistant, name, description, path) in matches {
        println!("{assistant}: {name}");
        println!("Description: {description}");
        println!("Path: {}", path.display());
        println!();
    }

    Ok(())
}

pub fn cmd_stats(cmd: &StatsCommand, config: &Config, paths: &AppPaths) -> Result<()> {
    let assistants = resolve_stats_assistants(&cmd.assistant, config);
    let usage = UsageStore::load(paths)?;
    let mut total_bytes = 0u64;
    let mut total_skills = 0u64;

    for assistant in &assistants {
        let root = config.skills_root_for(paths, *assistant);
        let mut skills = Vec::new();
        let mut assistant_bytes = 0u64;

        if root.exists() {
            for entry in
                fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))?
            {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let skill_dir = entry.path();
                    if skill_dir.join("SKILL.md").exists() {
                        if let Some(name) = skill_dir.file_name().and_then(|n| n.to_str()) {
                            let size = skill_size(&skill_dir)?;
                            assistant_bytes += size;
                            skills.push((name.to_string(), size));
                        }
                    }
                }
            }
        }

        skills.sort_by(|a, b| a.0.cmp(&b.0));
        total_bytes += assistant_bytes;
        total_skills += skills.len() as u64;

        println!("{assistant}:");
        println!("Skills: {}", skills.len());
        println!("Size: {}", ByteSize(assistant_bytes));

        let usage_total: u64 = skills
            .iter()
            .map(|(name, _)| usage.count_for(*assistant, name))
            .sum();
        if usage_total > 0 {
            println!("Usage: {}", usage_total);
            for (name, _) in &skills {
                let count = usage.count_for(*assistant, name);
                if count > 0 {
                    println!("  {name}: {count}");
                }
            }
        }

        println!();
    }

    if assistants.len() > 1 {
        println!("Total skills: {}", total_skills);
        println!("Total size: {}", ByteSize(total_bytes));
    }

    Ok(())
}

pub fn cmd_mark_used(cmd: &MarkUsedCommand, config: &Config, paths: &AppPaths) -> Result<()> {
    let assistant = resolve_single_assistant(&cmd.assistant, config, "mark-used")?;
    let mut store = UsageStore::load(paths)?;
    store.increment(assistant, &cmd.name);
    store.save(paths)?;
    println!("Marked {} used for {}", cmd.name, assistant);
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

fn resolve_stats_assistants(args: &AssistantArgs, config: &Config) -> Vec<Assistant> {
    if let Some(selected) = args.selected() {
        return vec![selected];
    }

    if let Some(default) = config.default_assistant {
        eprintln!(
            "Warning: using default assistant {default} for stats. Use --codex/--claudecode/--opencode to override."
        );
        return vec![default];
    }

    eprintln!("Warning: no default assistant set. Showing stats for all assistants.");
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

fn skill_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(path)?;
        if should_skip(rel_path) {
            continue;
        }

        if entry.file_type().is_file() {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

fn should_skip(rel_path: &Path) -> bool {
    rel_path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git") | Some("target") | Some(".DS_Store")
        )
    })
}
