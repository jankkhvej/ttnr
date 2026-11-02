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
# ${GEN_DURATION:-3} s of a 440 Hz sine, converted to the layout by aformat.
gen() {
  local file=$1 rate=$2 layout=$3
  shift 3
  ffmpeg -nostdin -hide_banner -loglevel error -y \
    -f lavfi -i "sine=frequency=440:sample_rate=${rate}:duration=${GEN_DURATION:-3}" \
    -af "aformat=channel_layouts=$layout" "$@" "$out/$file"
}

gen mp3.mp3           44100 stereo      -c:a libmp3lame -b:a 192k
gen mono.mp3          22050 mono        -c:a libmp3lame -b:a 64k
gen aac_lc.m4a        48000 stereo      -c:a aac -b:a 128k
gen aac_adts.aac      44100 stereo      -c:a aac -b:a 128k -f adts
gen he_aac.m4a        48000 stereo      -c:a aac_at -profile:a 4 -b:a 64k    # AV_PROFILE_AAC_HE
gen he_aac_v2.m4a     48000 stereo      -c:a aac_at -profile:a 28 -b:a 32k   # AV_PROFILE_AAC_HE_V2
# Raw MP3 frames without ID3/Xing headers: body of the loopback ICY server.
GEN_DURATION=10 gen icy_stream.mp3 44100 mono -c:a libmp3lame -b:a 64k -write_xing 0 -id3v2_version 0
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

# channel_check <file> <layout> <channel names in FFmpeg order…>
# Speaker-mapping check by ear: one channel at a time in file order, 1.5 s
# pink noise (~-22 dBFS peak) with 10 ms fades, 0.5 s gap; LFE gets a 60 Hz
# sine (~-20 dBFS; `sine` defaults to amplitude 1/8). Two cycles. Recipe
# after ALSA speaker-test / SMPTE ST 2095 practice (docs/RESEARCH.md);
# FFmpeg channel order, see `ffmpeg -layouts`.
channel_check() {
  local file=$1 layout=$2 i graph="" ins="" map=""
  shift 2
  local -a names=("$@")
  local n=${#names} on=1.5 slot=2 cycle=$(( ${#names} * 2 ))
  for (( i = 0; i < n; i++ )); do
    # join must be told the mapping: a mono input is FC and would go there.
    map+="${map:+|}$i.0-${names[i + 1]}"
    if [[ ${names[i + 1]} == LFE ]]; then
      graph+="sine=frequency=60:sample_rate=48000,volume=0.8,"
    else
      graph+="anoisesrc=color=pink:sample_rate=48000:amplitude=0.1:seed=$((i + 1)),"
    fi
    graph+="atrim=0:$on,afade=t=in:d=0.01,afade=t=out:st=$(( on - 0.01 )):d=0.01,"
    graph+="adelay=$(( i * slot * 1000 )),apad=whole_dur=${cycle}[c$i];"
    ins+="[c$i]"
  done
  graph+="${ins}join=inputs=${n}:channel_layout=${layout}:map=${map},aloop=loop=1:size=$(( cycle * 48000 ))"
  ffmpeg -nostdin -hide_banner -loglevel error -y -filter_complex "$graph" \
    -c:a flac -sample_fmt s16 "$out/$file"
}

channel_check channel_check_51.flac '5.1(side)' FL FR FC LFE SL SR
channel_check channel_check_71.flac 7.1         FL FR FC LFE BL BR SL SR

print -r -- $want > $stamp
print "gen-fixtures: wrote $out"
