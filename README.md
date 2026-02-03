# SkillSlash
/

SkillSlash (`skill`) is a cross-platform CLI for managing Agent Skills for Codex, Claude Code, and OpenCode.

## Installation

Homebrew (macOS):

```bash
brew tap dmoliveira/skill
brew install skill
```

Maintainers: update `packaging/homebrew/skill.rb` with release version + sha256.

Linux (tarball):

```bash
curl -fsSL https://github.com/dmoliveira/skill/releases/latest/download/skill-x86_64-unknown-linux-gnu.tar.gz \
  | tar -xz
sudo install skill /usr/local/bin/skill
```

Linux (install script):

```bash
curl -fsSL https://raw.githubusercontent.com/dmoliveira/skill/main/packaging/linux/install.sh \
  | bash
```

Windows (winget):

```powershell
winget install dmoliveira.skill
```

Maintainers: update `packaging/windows/winget/skill.yaml` and `packaging/windows/scoop/skill.json` on release.

Windows (zip):

```powershell
irm https://github.com/dmoliveira/skill/releases/latest/download/skill-x86_64-pc-windows-msvc.zip -OutFile skill.zip
Expand-Archive skill.zip -DestinationPath $env:ProgramFiles\skill
$env:Path += ";$env:ProgramFiles\skill"
```

## Quick start

```bash
skill default opencode
skill add ./my-skill --opencode
skill list --opencode
skill show my-skill
skill stats
```

## Safety

Skill usage is at your own risk. Always verify and trust the source before installing skills.
SkillSlash validates and scans skills before install and will prompt for confirmation.

## Commands

- `skill add <path|git-url|archive-url> [--codex|--claudecode|--opencode] [--yes]`: validate/scan and install a skill from a local dir, git repo, or archive URL; `--yes` skips confirmation. Archive URLs must end with `.zip`, `.tar`, `.tar.gz`, or `.tgz`.
- `skill remove <name> [--codex|--claudecode|--opencode] [--yes]`: uninstall a skill by name; `--yes` skips confirmation.
- `skill list [--codex|--claudecode|--opencode]`: list installed skills for one assistant (or default).
- `skill show <name> [--codex|--claudecode|--opencode]`: show metadata and path for a skill.
- `skill default <codex|claudecode|opencode>`: set the default assistant.
- `skill stats [--codex|--claudecode|--opencode]`: show counts, size, and usage for an assistant.
- `skill search <query> [--codex|--claudecode|--opencode]`: search installed skills by metadata and content.
- `skill scan <path>`: run security scan on a directory.
- `skill validate <path>`: validate `SKILL.md` and structure.
- `skill mark-used <name> [--codex|--claudecode|--opencode]`: increment usage counter.
- `skill paths`: show config and data directories.
- `skill --help` / `skill <cmd> --help`: show help for commands.

## Validation and scanning

- Validates `SKILL.md` against the Agent Skills spec.
- Scans for secrets, risky commands, and binary artifacts.
- Optional external scanners: `trivy` and `clamscan` if installed, plus `yara` when `SKILL_YARA_RULES` is set.

## Paths

Run `skill paths` to see the exact directories in use. Defaults:

- macOS/Linux: `~/.skills/data/<assistant>`
- Windows: `%USERPROFILE%\.skills\data\<assistant>`

Config file:

- macOS/Linux: `~/.skills/config.yaml`
- Windows: `%USERPROFILE%\.skills\config.yaml`

Config file location is shown by `skill paths`. A default config is bootstrapped
from `config.example.yaml` on first run.

## Development

```bash
cargo fmt
cargo clippy
cargo test
```
