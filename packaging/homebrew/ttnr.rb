# Homebrew formula for ttnr (ADR-044). Published through the tap
# github.com/jankkhvej/homebrew-ttnr as Formula/ttnr.rb:
#   brew tap jankkhvej/ttnr && brew trust jankkhvej/ttnr
#   brew install jankkhvej/ttnr/ttnr
# Checked 2026-11-04: `brew audit --strict --new --online`, install from
# source, `brew test` (Homebrew 7.0.12, macOS 27). v0.1.2, v0.1.3 (2026-11-06):
# `brew audit --strict --online` from the tap (no install).
# On a new release: update `url` to the tag's archive and `sha256` to
# `curl -sL <url> | shasum -a 256`.
class Ttnr < Formula
  desc "Minimal, themeable terminal player for network radio"
  homepage "https://github.com/jankkhvej/ttnr"
  url "https://github.com/jankkhvej/ttnr/archive/refs/tags/v0.1.3.tar.gz"
  sha256 "ca37ba5b3c537b1b141a19b70d6d5e05fbb937f6b336698ae58c64960fb2bcf1"
  # The source is BSD-2-Clause; the installed binary links Homebrew's GPL
  # builds of mpv and FFmpeg (README "Licence").
  license "BSD-2-Clause"
  head "https://github.com/jankkhvej/ttnr.git", branch: "main"

  depends_on "pkgconf" => :build
  depends_on "rust" => :build
  depends_on :macos # Now Playing / CoreAudio integration (SPEC non-goals)
  depends_on "mpv"

  def install
    # ttnr-engine's build script finds libmpv through pkg-config (mpv.pc).
    system "cargo", "install", *std_cargo_args
    man1.install "docs/ttnr.1"
    pkgshare.install "examples"
  end

  def caveats
    <<~EOS
      Example configuration and stations: #{opt_pkgshare}/examples
      Copy them to ~/.config/ttnr/ to start.

      Needs mpv 0.42 or newer (`brew upgrade mpv`): with mpv 0.41 CoreAudio
      output fails on macOS 27 and ttnr falls back to AVFoundation.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ttnr --version")
    # Runs without any configuration.
    ENV["XDG_CONFIG_HOME"] = testpath/"config"
    system bin/"ttnr", "stations"
  end
end
