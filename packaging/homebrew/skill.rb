class Skill < Formula
  desc "Manage Agent Skills for Codex, Claude Code, and OpenCode"
  homepage "https://github.com/dmoliveira/skill"
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/dmoliveira/skill/releases/download/v0.1.0/skill-aarch64-apple-darwin.tar.gz"
      sha256 "dfc39335cddf7104ff9575c5c517ef30d7537efc3b067c69f53efee862a5f4ba"
    else
      url "https://github.com/dmoliveira/skill/releases/download/v0.1.0/skill-x86_64-apple-darwin.tar.gz"
      sha256 "0cff8b074f8903ebfe3a0a9c3ede71b87e21f17d9180199f3b437e0ec3db17fd"
    end
  end

  def install
    bin.install "skill"
  end

  test do
    system "#{bin}/skill", "paths"
  end
end
