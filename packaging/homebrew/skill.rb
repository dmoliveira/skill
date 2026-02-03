class Skill < Formula
  desc "Manage Agent Skills for Codex, Claude Code, and OpenCode"
  homepage "https://github.com/dmoliveira/skill"
  version "0.1.0"
  url "https://github.com/dmoliveira/skill/releases/download/v0.1.0/skill-x86_64-apple-darwin.tar.gz"
  sha256 "REPLACE_ME"

  def install
    bin.install "skill"
  end

  test do
    system "#{bin}/skill", "paths"
  end
end
