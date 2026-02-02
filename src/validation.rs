use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub message: String,
    pub path: Option<PathBuf>,
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        if let Some(path) = &self.path {
            write!(f, "[{level}] {} ({})", self.message, path.display())
        } else {
            write!(f, "[{level}] {}", self.message)
        }
    }
}

#[derive(Debug, Default)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == Severity::Error)
    }
}

#[derive(Debug, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
}

pub fn validate_skill_dir(path: &Path) -> Result<ValidationReport> {
    let mut report = ValidationReport::default();

    if !path.exists() {
        return Err(anyhow!("path does not exist: {}", path.display()));
    }

    if !path.is_dir() {
        report.issues.push(ValidationIssue {
            severity: Severity::Error,
            message: "skill path must be a directory".to_string(),
            path: Some(path.to_path_buf()),
        });
        return Ok(report);
    }

    let skill_md_path = path.join("SKILL.md");
    if !skill_md_path.exists() {
        report.issues.push(ValidationIssue {
            severity: Severity::Error,
            message: "SKILL.md is missing".to_string(),
            path: Some(skill_md_path),
        });
        return Ok(report);
    }

    let frontmatter = match read_frontmatter(path) {
        Ok(frontmatter) => frontmatter,
        Err(err) => {
            report.issues.push(ValidationIssue {
                severity: Severity::Error,
                message: err.to_string(),
                path: Some(skill_md_path),
            });
            return Ok(report);
        }
    };

    validate_name(&frontmatter.name, path, &mut report);
    validate_description(&frontmatter.description, &mut report, &skill_md_path);
    validate_optional_field(
        "license",
        &frontmatter.license,
        256,
        &mut report,
        &skill_md_path,
    );
    validate_optional_field(
        "compatibility",
        &frontmatter.compatibility,
        500,
        &mut report,
        &skill_md_path,
    );
    validate_optional_field(
        "allowed-tools",
        &frontmatter.allowed_tools,
        2048,
        &mut report,
        &skill_md_path,
    );

    if let Some(metadata) = &frontmatter.metadata {
        for (key, value) in metadata {
            if key.trim().is_empty() || value.trim().is_empty() {
                report.issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    message: "metadata entries should not be empty".to_string(),
                    path: Some(skill_md_path.clone()),
                });
                break;
            }
        }
    }

    Ok(report)
}

pub fn read_frontmatter(path: &Path) -> Result<SkillFrontmatter> {
    let skill_md_path = path.join("SKILL.md");
    let contents = fs::read_to_string(&skill_md_path)
        .with_context(|| format!("failed to read {}", skill_md_path.display()))?;
    parse_frontmatter(&contents).map_err(|err| anyhow!("invalid frontmatter: {err}"))
}

fn parse_frontmatter(contents: &str) -> Result<SkillFrontmatter, String> {
    let mut lines = contents.lines();
    let first = lines.next().unwrap_or("").trim();
    if first != "---" {
        return Err("SKILL.md must start with YAML frontmatter (---)".to_string());
    }

    let mut yaml_lines = Vec::new();
    for line in lines.by_ref() {
        if line.trim() == "---" {
            break;
        }
        yaml_lines.push(line);
    }

    if yaml_lines.is_empty() {
        return Err("SKILL.md frontmatter is empty".to_string());
    }

    let yaml = yaml_lines.join("\n");
    serde_yaml::from_str(&yaml).map_err(|err| format!("{err}"))
}

fn validate_name(name: &str, path: &Path, report: &mut ValidationReport) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        report.issues.push(ValidationIssue {
            severity: Severity::Error,
            message: "name is required".to_string(),
            path: Some(path.to_path_buf()),
        });
        return;
    }

    if trimmed.len() > 64 {
        report.issues.push(ValidationIssue {
            severity: Severity::Error,
            message: "name must be <= 64 characters".to_string(),
            path: Some(path.to_path_buf()),
        });
    }

    let pattern = Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").expect("valid name regex");
    if !pattern.is_match(trimmed) {
        report.issues.push(ValidationIssue {
            severity: Severity::Error,
            message: "name must be lowercase alphanumeric with hyphens".to_string(),
            path: Some(path.to_path_buf()),
        });
    }

    if trimmed.contains("--") {
        report.issues.push(ValidationIssue {
            severity: Severity::Error,
            message: "name must not contain consecutive hyphens".to_string(),
            path: Some(path.to_path_buf()),
        });
    }

    if let Some(dir_name) = path.file_name().and_then(|name| name.to_str()) {
        if dir_name != trimmed {
            report.issues.push(ValidationIssue {
                severity: Severity::Error,
                message: "name must match the skill directory name".to_string(),
                path: Some(path.to_path_buf()),
            });
        }
    }
}

fn validate_description(description: &str, report: &mut ValidationReport, path: &Path) {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        report.issues.push(ValidationIssue {
            severity: Severity::Error,
            message: "description is required".to_string(),
            path: Some(path.to_path_buf()),
        });
        return;
    }

    if trimmed.len() > 1024 {
        report.issues.push(ValidationIssue {
            severity: Severity::Error,
            message: "description must be <= 1024 characters".to_string(),
            path: Some(path.to_path_buf()),
        });
    }
}

fn validate_optional_field(
    field: &str,
    value: &Option<String>,
    max_len: usize,
    report: &mut ValidationReport,
    path: &Path,
) {
    if let Some(value) = value {
        if value.trim().is_empty() {
            report.issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!("{field} should not be empty"),
                path: Some(path.to_path_buf()),
            });
        } else if value.len() > max_len {
            report.issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("{field} must be <= {max_len} characters"),
                path: Some(path.to_path_buf()),
            });
        }
    }
}
