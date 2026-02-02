# SkillSlash
/

SkillSlash (`skill`) is a cross-platform CLI for managing Agent Skills for Codex, Claude Code, and OpenCode.

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

- `skill add <path|git-url> [--codex|--claudecode|--opencode] [--yes]`
- `skill remove <name> [--codex|--claudecode|--opencode] [--yes]`
- `skill list [--codex|--claudecode|--opencode]`
- `skill show <name> [--codex|--claudecode|--opencode]`
- `skill default <codex|claudecode|opencode>`
- `skill stats [--codex|--claudecode|--opencode]`
- `skill search <query> [--codex|--claudecode|--opencode]`
- `skill scan <path>`
- `skill validate <path>`
- `skill mark-used <name> [--codex|--claudecode|--opencode]`
- `skill paths`

## Validation and scanning

- Validates `SKILL.md` against the Agent Skills spec.
- Scans for secrets, risky commands, and binary artifacts.
- Optional external scanners: `trivy` and `clamscan` if installed.

## Paths

Run `skill paths` to see the exact directories in use. Defaults:

- macOS: `~/Library/Application Support/AgentSkills/<assistant>`
- Linux: `~/.config/AgentSkills/<assistant>`
- Windows: `%APPDATA%\AgentSkills\<assistant>`

Config file location is platform-specific and shown by `skill paths`.

## Development

```bash
cargo fmt
cargo clippy
cargo test
```
