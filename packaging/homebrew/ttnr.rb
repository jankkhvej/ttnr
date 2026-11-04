# Homebrew formula for ttnr (ADR-044). Published through the tap
# github.com/jankkhvej/homebrew-ttnr as Formula/ttnr.rb:
#   brew tap jankkhvej/ttnr && brew trust jankkhvej/ttnr
#   brew install jankkhvej/ttnr/ttnr
# Checked 2026-11-04: `brew audit --strict --new --online`, install from
# source, `brew test` (Homebrew 7.0.12, macOS 27).
# On a new release: update `url` to the tag's archive and `sha256` to
# `curl -sL <url> | shasum -a 256`.
class Ttnr < Formula
  desc "Minimal, themeable terminal player for network radio"
  homepage "https://github.com/jankkhvej/ttnr"
  url "https://github.com/jankkhvej/ttnr/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "bfeea6ab9071612a7de6d3c200ac24b983f66b53e1dfedeb702c26457b2364e6"
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

      mpv 0.41.0's CoreAudio output fails on macOS 27 for many streams; ttnr
      then uses AVFoundation, where volume and mute react up to ~2 s late.
      This goes away with mpv 0.42 (upstream fix mpv-player/mpv#18679).
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ttnr --version")
    # Runs without any configuration.
    ENV["XDG_CONFIG_HOME"] = testpath/"config"
    system bin/"ttnr", "stations"
  end
end
