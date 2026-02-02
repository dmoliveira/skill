use crate::assistant::Assistant;
use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "skill", version, about = "Manage Agent Skills", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Paths(PathsCommand),
    Add(AddCommand),
    Remove(RemoveCommand),
    List(ListCommand),
    Show(ShowCommand),
    Default(DefaultCommand),
    Stats(StatsCommand),
    Search(SearchCommand),
    Scan(ScanCommand),
    Validate(ValidateCommand),
    MarkUsed(MarkUsedCommand),
}

#[derive(Args, Debug, Clone, Default)]
pub struct AssistantArgs {
    #[arg(long, conflicts_with_all = ["claudecode", "opencode"]) ]
    pub codex: bool,
    #[arg(long, conflicts_with_all = ["codex", "opencode"]) ]
    pub claudecode: bool,
    #[arg(long, conflicts_with_all = ["codex", "claudecode"]) ]
    pub opencode: bool,
}

impl AssistantArgs {
    pub fn selected(&self) -> Option<Assistant> {
        if self.codex {
            Some(Assistant::Codex)
        } else if self.claudecode {
            Some(Assistant::ClaudeCode)
        } else if self.opencode {
            Some(Assistant::OpenCode)
        } else {
            None
        }
    }
}

#[derive(Args, Debug)]
pub struct PathsCommand {
    #[command(flatten)]
    pub assistant: AssistantArgs,
}

#[derive(Args, Debug)]
pub struct AddCommand {
    #[command(flatten)]
    pub assistant: AssistantArgs,
    pub source: String,
    #[arg(long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct RemoveCommand {
    #[command(flatten)]
    pub assistant: AssistantArgs,
    pub name: String,
}

#[derive(Args, Debug)]
pub struct ListCommand {
    #[command(flatten)]
    pub assistant: AssistantArgs,
}

#[derive(Args, Debug)]
pub struct ShowCommand {
    #[command(flatten)]
    pub assistant: AssistantArgs,
    pub name: String,
}

#[derive(Args, Debug)]
pub struct DefaultCommand {
    pub assistant: Assistant,
}

#[derive(Args, Debug)]
pub struct StatsCommand {
    #[command(flatten)]
    pub assistant: AssistantArgs,
}

#[derive(Args, Debug)]
pub struct SearchCommand {
    #[command(flatten)]
    pub assistant: AssistantArgs,
    pub query: String,
}

#[derive(Args, Debug)]
pub struct ScanCommand {
    pub path: String,
}

#[derive(Args, Debug)]
pub struct ValidateCommand {
    pub path: String,
}

#[derive(Args, Debug)]
pub struct MarkUsedCommand {
    #[command(flatten)]
    pub assistant: AssistantArgs,
    pub name: String,
}
