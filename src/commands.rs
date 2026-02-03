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
use flate2::read::GzDecoder;
use std::fs;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::ZipArchive;

pub fn cmd_add(cmd: &AddCommand, config: &Config, paths: &AppPaths) -> Result<()> {
    let assistant = resolve_single_assistant(&cmd.assistant, config, "add")?;
    let (source_dir, temp_dir) = prepare_source(&cmd.source)?;
    let skill_dir = match cmd.skill.as_deref() {
        Some(skill) => resolve_skill_path(&source_dir, skill)?,
        None => source_dir,
    };

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
        if temp_dir.is_some() {
            eprintln!("Downloaded files were removed after scan failure.");
        }
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

    if looks_like_http_url(source) {
        if let Some(archive_type) = detect_archive_type(source) {
            let (path, temp_dir) = download_and_extract(source, archive_type)?;
            return Ok((path, Some(temp_dir)));
        }
        let (path, temp_dir) = clone_git_source(source)?;
        return Ok((path, Some(temp_dir)));
    }

    if looks_like_git_source(source) {
        let (path, temp_dir) = clone_git_source(source)?;
        return Ok((path, Some(temp_dir)));
    }

    Err(anyhow!("source not found: {source}"))
}

fn resolve_skill_path(root: &Path, skill: &str) -> Result<PathBuf> {
    let skill_path = Path::new(skill);
    if skill_path.is_absolute() {
        return Err(anyhow!("--skill must be a relative path"));
    }
    if skill_path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(anyhow!("--skill must not contain '..'"));
    }

    let mut candidates = Vec::new();
    candidates.push(root.join(skill_path));
    if !skill_path.starts_with("skills") {
        candidates.push(root.join("skills").join(skill_path));
    }
    if !skill_path.starts_with("skill") {
        candidates.push(root.join("skill").join(skill_path));
    }

    for candidate in candidates {
        if candidate.is_dir() && candidate.join("SKILL.md").exists() {
            return Ok(candidate);
        }
    }

    Err(anyhow!(
        "skill '{skill}' not found. Expected SKILL.md in <repo>/{skill}, <repo>/skills/{skill}, or <repo>/skill/{skill}"
    ))
}

fn clone_git_source(source: &str) -> Result<(PathBuf, TempDir)> {
    let temp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    let status = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(source)
        .arg(temp_dir.path())
        .status()
        .with_context(|| format!("failed to run git clone for {source}"))?;

    if !status.success() {
        return Err(anyhow!("git clone failed for {source}"));
    }

    Ok((temp_dir.path().to_path_buf(), temp_dir))
}

fn looks_like_git_source(source: &str) -> bool {
    source.starts_with("git@")
        || source.starts_with("ssh://")
        || source.starts_with("git://")
        || source.ends_with(".git")
}

fn looks_like_http_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

#[derive(Debug, Clone, Copy)]
enum ArchiveType {
    Zip,
    Tar,
    TarGz,
}

const MAX_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 5_000;

fn detect_archive_type(source: &str) -> Option<ArchiveType> {
    let lower = source.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        Some(ArchiveType::Zip)
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        Some(ArchiveType::TarGz)
    } else if lower.ends_with(".tar") {
        Some(ArchiveType::Tar)
    } else {
        None
    }
}

fn download_and_extract(url: &str, archive_type: ArchiveType) -> Result<(PathBuf, TempDir)> {
    let temp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    let archive_name = match archive_type {
        ArchiveType::Zip => "skill.zip",
        ArchiveType::Tar => "skill.tar",
        ArchiveType::TarGz => "skill.tar.gz",
    };
    let archive_path = temp_dir.path().join(archive_name);
    let response = ureq::get(url)
        .call()
        .map_err(|err| anyhow!("failed to download {url}: {err}"))?;
    validate_content_type(archive_type, response.header("Content-Type"))?;
    if let Some(length) = response.header("Content-Length") {
        if let Ok(size) = length.parse::<u64>() {
            if size > MAX_DOWNLOAD_BYTES {
                return Err(anyhow!(
                    "download too large ({size} bytes). Limit is {MAX_DOWNLOAD_BYTES} bytes."
                ));
            }
        }
    }
    let mut reader = response.into_reader();
    let mut file = File::create(&archive_path)
        .with_context(|| format!("failed to create {}", archive_path.display()))?;
    copy_with_limit(&mut reader, &mut file, MAX_DOWNLOAD_BYTES).with_context(|| {
        format!(
            "failed to write downloaded archive {}",
            archive_path.display()
        )
    })?;

    let extract_dir = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_dir)
        .with_context(|| format!("failed to create {}", extract_dir.display()))?;

    match archive_type {
        ArchiveType::Zip => extract_zip(&archive_path, &extract_dir)?,
        ArchiveType::Tar => extract_tar(&archive_path, &extract_dir)?,
        ArchiveType::TarGz => extract_tar_gz(&archive_path, &extract_dir)?,
    }

    let skill_root = resolve_skill_root(&extract_dir)?;
    Ok((skill_root, temp_dir))
}

fn resolve_skill_root(extract_dir: &Path) -> Result<PathBuf> {
    if extract_dir.join("SKILL.md").exists() {
        return Ok(extract_dir.to_path_buf());
    }

    let mut found: Option<PathBuf> = None;
    for entry in WalkDir::new(extract_dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "SKILL.md" {
            continue;
        }
        let rel_path = entry.path().strip_prefix(extract_dir)?;
        if should_skip(rel_path) {
            continue;
        }
        let Some(parent) = entry.path().parent() else {
            continue;
        };
        let parent = parent.to_path_buf();
        if let Some(existing) = &found {
            if existing != &parent {
                return Err(anyhow!(
                    "archive contains multiple SKILL.md files; use an archive with a single skill"
                ));
            }
        } else {
            found = Some(parent);
        }
    }

    found.ok_or_else(|| anyhow!("archive did not contain a SKILL.md file"))
}

fn extract_zip(archive_path: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("failed to read {}", archive_path.display()))?;
    let entries = archive.len();
    if entries > MAX_ARCHIVE_ENTRIES {
        return Err(anyhow!("archive has too many entries ({entries})"));
    }

    let mut extracted = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("failed to read entry {i}"))?;
        let name = entry.name().to_string();
        let entry_path = Path::new(&name);
        let safe_path = sanitize_archive_path(entry_path)
            .with_context(|| format!("unsafe archive path: {name}"))?;

        if is_zip_symlink(&entry) {
            return Err(anyhow!("archive contains symlink: {name}"));
        }

        if entry.is_dir() {
            fs::create_dir_all(dest.join(&safe_path))?;
            continue;
        }

        let size = entry.size();
        extracted = extracted.saturating_add(size);
        if extracted > MAX_EXTRACTED_BYTES {
            return Err(anyhow!("extracted data exceeds limit"));
        }

        let out_path = dest.join(&safe_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&out_path)
            .with_context(|| format!("failed to create {}", out_path.display()))?;
        copy_with_limit(&mut entry, &mut output, MAX_EXTRACTED_BYTES)?;
    }
    Ok(())
}

fn extract_tar(archive_path: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    extract_tar_stream(file, dest)?;
    Ok(())
}

fn extract_tar_gz(archive_path: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    let decoder = GzDecoder::new(file);
    extract_tar_stream(decoder, dest)?;
    Ok(())
}

fn extract_tar_stream<R: Read>(reader: R, dest: &Path) -> Result<()> {
    let mut archive = Archive::new(reader);
    let mut extracted = 0u64;
    let mut entries = 0usize;

    for entry in archive.entries()? {
        let mut entry = entry?;
        entries += 1;
        if entries > MAX_ARCHIVE_ENTRIES {
            return Err(anyhow!("archive has too many entries ({entries})"));
        }

        let path = entry.path()?.into_owned();
        let safe_path = sanitize_archive_path(&path)
            .with_context(|| format!("unsafe archive path: {}", path.display()))?;

        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(anyhow!("archive contains link: {}", path.display()));
        }

        let size = entry.header().size().unwrap_or(0);
        extracted = extracted.saturating_add(size);
        if extracted > MAX_EXTRACTED_BYTES {
            return Err(anyhow!("extracted data exceeds limit"));
        }

        let out_path = dest.join(&safe_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(&out_path)?;
    }
    Ok(())
}

fn sanitize_archive_path(path: &Path) -> Result<PathBuf> {
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => safe.push(part),
            std::path::Component::CurDir => continue,
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(anyhow!("path traversal detected"));
            }
        }
    }
    Ok(safe)
}

fn is_zip_symlink(entry: &zip::read::ZipFile<'_>) -> bool {
    if let Some(mode) = entry.unix_mode() {
        let file_type = mode & 0o170000;
        return file_type == 0o120000;
    }
    false
}

fn validate_content_type(archive_type: ArchiveType, content_type: Option<&str>) -> Result<()> {
    let Some(content_type) = content_type else {
        return Ok(());
    };
    let content_type = content_type.to_ascii_lowercase();
    let allowed: &[&str] = match archive_type {
        ArchiveType::Zip => &[
            "application/zip",
            "application/octet-stream",
            "application/x-zip-compressed",
        ],
        ArchiveType::Tar => &["application/x-tar", "application/octet-stream"],
        ArchiveType::TarGz => &[
            "application/gzip",
            "application/x-gzip",
            "application/octet-stream",
        ],
    };

    if allowed.iter().any(|item| content_type.starts_with(item)) {
        return Ok(());
    }

    Err(anyhow!(
        "unsupported content-type for archive: {content_type}"
    ))
}

fn copy_with_limit<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    max_bytes: u64,
) -> Result<u64> {
    let mut total = 0u64;
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > max_bytes {
            return Err(anyhow!("data exceeds limit ({max_bytes} bytes)"));
        }
        writer.write_all(&buffer[..read])?;
    }
    Ok(total)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_skill(dir: &Path, name: &str) -> PathBuf {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\ndescription: test\n---\n",
        )
        .expect("write skill md");
        skill_dir
    }

    #[test]
    fn detect_archive_type_accepts_supported_extensions() {
        assert!(matches!(
            detect_archive_type("https://example.com/skill.zip"),
            Some(ArchiveType::Zip)
        ));
        assert!(matches!(
            detect_archive_type("https://example.com/skill.tar"),
            Some(ArchiveType::Tar)
        ));
        assert!(matches!(
            detect_archive_type("https://example.com/skill.tar.gz"),
            Some(ArchiveType::TarGz)
        ));
        assert!(matches!(
            detect_archive_type("https://example.com/skill.TGZ"),
            Some(ArchiveType::TarGz)
        ));
    }

    #[test]
    fn resolve_skill_root_uses_root_when_present() {
        let temp = tempdir().expect("temp dir");
        fs::write(
            temp.path().join("SKILL.md"),
            "---\nname: root-skill\ndescription: test\n---\n",
        )
        .expect("write skill md");

        let resolved = resolve_skill_root(temp.path()).expect("resolve root");
        assert_eq!(resolved, temp.path().to_path_buf());
    }

    #[test]
    fn resolve_skill_root_accepts_single_nested_skill() {
        let temp = tempdir().expect("temp dir");
        let nested = write_skill(temp.path(), "nested-skill");

        let resolved = resolve_skill_root(temp.path()).expect("resolve root");
        assert_eq!(resolved, nested);
    }

    #[test]
    fn resolve_skill_root_rejects_multiple_skills() {
        let temp = tempdir().expect("temp dir");
        write_skill(temp.path(), "skill-one");
        write_skill(temp.path(), "skill-two");

        let result = resolve_skill_root(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn resolve_skill_root_errors_when_missing() {
        let temp = tempdir().expect("temp dir");

        let result = resolve_skill_root(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn resolve_skill_path_uses_direct_match() {
        let temp = tempdir().expect("temp dir");
        let direct = write_skill(temp.path(), "direct-skill");

        let resolved = resolve_skill_path(temp.path(), "direct-skill").expect("resolve skill");
        assert_eq!(resolved, direct);
    }

    #[test]
    fn resolve_skill_path_falls_back_to_skills_dir() {
        let temp = tempdir().expect("temp dir");
        let skills_dir = temp.path().join("skills");
        fs::create_dir_all(&skills_dir).expect("create skills dir");
        let nested = write_skill(&skills_dir, "nested-skill");

        let resolved = resolve_skill_path(temp.path(), "nested-skill").expect("resolve skill");
        assert_eq!(resolved, nested);
    }

    #[test]
    fn resolve_skill_path_falls_back_to_skill_dir() {
        let temp = tempdir().expect("temp dir");
        let skills_dir = temp.path().join("skill");
        fs::create_dir_all(&skills_dir).expect("create skill dir");
        let nested = write_skill(&skills_dir, "nested-skill");

        let resolved = resolve_skill_path(temp.path(), "nested-skill").expect("resolve skill");
        assert_eq!(resolved, nested);
    }

    #[test]
    fn resolve_skill_path_rejects_parent_dirs() {
        let temp = tempdir().expect("temp dir");

        let result = resolve_skill_path(temp.path(), "../escape");
        assert!(result.is_err());
    }
}
