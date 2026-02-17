class Wakalyze < Formula
  desc "CLI to list Wakapi working hours per day using the heartbeats API"
  homepage "https://github.com/masaishi/wakalyze"
  version "0.1.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/masaishi/wakalyze/releases/download/v#{version}/wakalyze-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    end
    on_intel do
      url "https://github.com/masaishi/wakalyze/releases/download/v#{version}/wakalyze-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/masaishi/wakalyze/releases/download/v#{version}/wakalyze-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  def install
    bin.install "wakalyze"
  end

  test do
    assert_match "wakalyze", shell_output("#{bin}/wakalyze --help")
  end
end
