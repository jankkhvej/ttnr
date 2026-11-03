# DRAFT – not published, not tested with `brew install` (ADR-044).
# Needs a public source archive: replace URL and SHA-256, then put this file
# in a tap (Homebrew installs formulae from taps, e.g. one made with
# `brew tap-new <user>/<tap>`).
class Ttnr < Formula
  desc "Minimal, themeable terminal player for network radio"
  homepage "https://hcc07.org/ttnr" # UNVERIFIED: the project page is not online yet
  url "https://example.invalid/ttnr-0.1.0.tar.gz" # TODO: real release archive
  sha256 "0000000000000000000000000000000000000000000000000000000000000000" # TODO
  # The source is BSD-2-Clause; the installed binary links Homebrew's GPL
  # builds of mpv and FFmpeg (README "Licence").
  license "BSD-2-Clause"

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
