use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
pub struct ScanIssue {
    pub severity: Severity,
    pub message: String,
    pub path: Option<PathBuf>,
}

impl fmt::Display for ScanIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        if let Some(path) = &self.path {
            write!(f, "[{level}] {} ({})", self.message, path.display())
        } else {
            write!(f, "[{level}] {}", self.message)
        }
    }
}

#[derive(Debug, Default)]
pub struct ScanReport {
    pub issues: Vec<ScanIssue>,
    pub external: Vec<ExternalScan>,
}

impl ScanReport {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == Severity::Error)
            || self
                .external
                .iter()
                .any(|scan| scan.severity == Severity::Error)
    }
}

#[derive(Debug, Clone)]
pub struct ExternalScan {
    pub tool: String,
    pub severity: Severity,
    pub output: String,
}

static SECRET_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"AKIA[0-9A-Z]{16}").expect("aws key regex"),
        Regex::new(r"ASIA[0-9A-Z]{16}").expect("aws key regex"),
        Regex::new(r"ghp_[A-Za-z0-9]{36,}").expect("github token regex"),
        Regex::new(r"xox[baprs]-[A-Za-z0-9-]{10,}").expect("slack token regex"),
        Regex::new(r"-----BEGIN (RSA|OPENSSH|EC|PGP) PRIVATE KEY-----").expect("private key regex"),
    ]
});

static DANGEROUS_COMMANDS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"rm\s+-rf\s+/").expect("rm -rf regex"),
        Regex::new(r"curl\s+[^\n]+\|\s*sh").expect("curl|sh regex"),
        Regex::new(r"wget\s+[^\n]+\|\s*sh").expect("wget|sh regex"),
        Regex::new(r"chmod\s+777").expect("chmod 777 regex"),
        Regex::new(r"sudo\s+").expect("sudo regex"),
    ]
});

const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

pub fn scan_path(path: &Path) -> Result<ScanReport> {
    let mut report = ScanReport::default();

    if !path.exists() {
        return Err(anyhow!("path does not exist: {}", path.display()));
    }

    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry?;
        let entry_path = entry.path();

        if entry.file_type().is_symlink() {
            report.issues.push(ScanIssue {
                severity: Severity::Warning,
                message: "symlink detected".to_string(),
                path: Some(entry_path.to_path_buf()),
            });
            continue;
        }

        if entry.file_type().is_dir() {
            continue;
        }

        let metadata = entry.metadata()?;
        if metadata.len() > MAX_FILE_BYTES {
            report.issues.push(ScanIssue {
                severity: Severity::Warning,
                message: format!("large file ({} bytes)", metadata.len()),
                path: Some(entry_path.to_path_buf()),
            });
        }

        if let Some(ext) = entry_path.extension().and_then(|ext| ext.to_str()) {
            if matches!(
                ext.to_ascii_lowercase().as_str(),
                "exe" | "dll" | "dylib" | "so" | "bat" | "cmd" | "ps1"
            ) {
                report.issues.push(ScanIssue {
                    severity: Severity::Warning,
                    message: "executable or binary file detected".to_string(),
                    path: Some(entry_path.to_path_buf()),
                });
            }
        }

        let bytes = fs::read(entry_path)
            .with_context(|| format!("failed to read {}", entry_path.display()))?;

        if bytes.contains(&0) {
            report.issues.push(ScanIssue {
                severity: Severity::Warning,
                message: "binary content detected".to_string(),
                path: Some(entry_path.to_path_buf()),
            });
            continue;
        }

        let Ok(content) = std::str::from_utf8(&bytes) else {
            report.issues.push(ScanIssue {
                severity: Severity::Warning,
                message: "non-utf8 file content detected".to_string(),
                path: Some(entry_path.to_path_buf()),
            });
            continue;
        };

        for pattern in SECRET_PATTERNS.iter() {
            if pattern.is_match(content) {
                report.issues.push(ScanIssue {
                    severity: Severity::Error,
                    message: "potential secret detected".to_string(),
                    path: Some(entry_path.to_path_buf()),
                });
                break;
            }
        }

        if is_script(entry_path) {
            for pattern in DANGEROUS_COMMANDS.iter() {
                if pattern.is_match(content) {
                    report.issues.push(ScanIssue {
                        severity: Severity::Warning,
                        message: "risky command detected in script".to_string(),
                        path: Some(entry_path.to_path_buf()),
                    });
                    break;
                }
            }
        }
    }

    report.external.extend(run_external_scans(path)?);
    Ok(report)
}

fn is_script(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd" | "py" | "js" | "ts"
    )
}

fn run_external_scans(path: &Path) -> Result<Vec<ExternalScan>> {
    let mut scans = Vec::new();

    if which::which("trivy").is_ok() {
        let output = Command::new("trivy")
            .arg("fs")
            .arg("--quiet")
            .arg(path)
            .output()
            .with_context(|| "failed to run trivy fs")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}{}", stdout, stderr).trim().to_string();
        let severity = if output.status.success() {
            Severity::Info
        } else {
            Severity::Warning
        };

        scans.push(ExternalScan {
            tool: "trivy".to_string(),
            severity,
            output: if combined.is_empty() {
                "trivy produced no output".to_string()
            } else {
                combined
            },
        });
    }

    if which::which("clamscan").is_ok() {
        let output = Command::new("clamscan")
            .arg("-r")
            .arg(path)
            .output()
            .with_context(|| "failed to run clamscan")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}{}", stdout, stderr).trim().to_string();
        let severity = if output.status.success() {
            Severity::Info
        } else {
            Severity::Warning
        };

        scans.push(ExternalScan {
            tool: "clamscan".to_string(),
            severity,
            output: if combined.is_empty() {
                "clamscan produced no output".to_string()
            } else {
                combined
            },
        });
    }

    Ok(scans)
}
