#!/usr/bin/env zsh
# Generate short audio fixtures for the format-matrix test (ROADMAP M1).
#
#   scripts/gen-fixtures.zsh            generate if missing or this script changed
#   scripts/gen-fixtures.zsh --force    regenerate unconditionally
#
# Output: tests/fixtures/generated/ (git-ignored). Needs Homebrew ffmpeg on
# macOS: HE-AAC comes from the AudioToolbox encoder `aac_at` (docs/RESEARCH.md).
# Fails loudly if an encoder is missing – the matrix must never shrink silently.
emulate -L zsh
setopt err_exit pipe_fail no_unset
cd "${0:A:h}/.."

out=tests/fixtures/generated
stamp=$out/.stamp
want=$(shasum -a 256 "${0:A}" | cut -d' ' -f1)
if [[ ${1:-} != --force && -f $stamp && "$(<$stamp)" == $want ]]; then
  exit 0
fi

command -v ffmpeg >/dev/null || { print -u2 "gen-fixtures: ffmpeg not found (brew install ffmpeg)"; exit 1; }
encoders=$(ffmpeg -hide_banner -encoders 2>/dev/null)
for enc in libmp3lame aac aac_at libopus vorbis flac alac ac3 eac3 pcm_s24le; do
  [[ $encoders == *" $enc "* ]] || { print -u2 "gen-fixtures: ffmpeg lacks encoder '$enc'"; exit 1; }
done

rm -rf $out
mkdir -p $out/hls $out/dash

# gen <file> <sample rate> <channel layout> <ffmpeg output options…>
# 3 s of a 440 Hz sine, converted to the layout by aformat.
gen() {
  local file=$1 rate=$2 layout=$3
  shift 3
  ffmpeg -nostdin -hide_banner -loglevel error -y \
    -f lavfi -i "sine=frequency=440:sample_rate=$rate:duration=3" \
    -af "aformat=channel_layouts=$layout" "$@" "$out/$file"
}

gen mp3.mp3           44100 stereo      -c:a libmp3lame -b:a 192k
gen mono.mp3          22050 mono        -c:a libmp3lame -b:a 64k
gen aac_lc.m4a        48000 stereo      -c:a aac -b:a 128k
gen aac_adts.aac      44100 stereo      -c:a aac -b:a 128k -f adts
gen he_aac.m4a        48000 stereo      -c:a aac_at -profile:a 4 -b:a 64k    # AV_PROFILE_AAC_HE
gen he_aac_v2.m4a     48000 stereo      -c:a aac_at -profile:a 28 -b:a 32k   # AV_PROFILE_AAC_HE_V2
gen opus.opus         48000 stereo      -c:a libopus -b:a 128k
gen vorbis.ogg        44100 stereo      -c:a vorbis -strict experimental
gen flac_16_44.flac   44100 stereo      -c:a flac -sample_fmt s16
gen flac_24_96.flac   96000 stereo      -c:a flac -sample_fmt s32 -bits_per_raw_sample 24
gen alac_24.m4a       48000 stereo      -c:a alac -sample_fmt s32p
gen pcm_s24.wav       48000 stereo      -c:a pcm_s24le
gen ac3_51.ac3        48000 '5.1(side)' -c:a ac3 -b:a 448k
gen eac3_51.eac3      48000 '5.1(side)' -c:a eac3 -b:a 384k              # FFmpeg's eac3 encoder stops at 5.1
gen aac_71.m4a        48000 7.1         -c:a aac -b:a 384k
gen opus_71.opus      48000 7.1         -c:a libopus -b:a 256k
gen hls/index.m3u8    48000 stereo      -c:a aac -b:a 96k -f hls -hls_time 1 \
                                        -hls_playlist_type vod -hls_segment_filename "$out/hls/seg%02d.ts"
gen dash/manifest.mpd 48000 stereo      -c:a aac -b:a 96k -f dash -seg_duration 1

print -r -- $want > $stamp
print "gen-fixtures: wrote $out"
