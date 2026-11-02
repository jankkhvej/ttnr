#!/usr/bin/env zsh
# Build a local libmpv with the CoreAudio fix that Homebrew's mpv 0.41.0
# lacks (mpv #18384 / PR #18679; ADR-015). Nothing is installed system-wide:
# everything lives in third_party/mpv/ (git-ignored) and the engine's
# build.rs prefers third_party/mpv/prefix over Homebrew's libmpv.
#
#   scripts/build-libmpv.zsh            build if missing or this script changed
#   scripts/build-libmpv.zsh --force    rebuild unconditionally
#
# Recipe = Homebrew's formula (v0.41.0 + its two backports) + PR #18679,
# libmpv only, without features ttnr never uses. Needs the Homebrew
# libraries mpv depends on (ffmpeg, libplacebo, libass, …: `brew install mpv`
# provides them), Xcode command line tools, python3, network for downloads.
emulate -L zsh
setopt err_exit pipe_fail no_unset

root=${0:A:h:h}
dir=$root/third_party/mpv
prefix=$dir/prefix
stamp=$prefix/.stamp
want=$(shasum -a 256 "${0:A}" | cut -d' ' -f1)
if [[ ${1:-} != --force && -f $stamp && "$(<$stamp)" == $want ]]; then
  print "build-libmpv: up to date ($prefix)"
  exit 0
fi

# name  url  sha256 – sources verified 2026-11-02 (docs/RESEARCH.md)
downloads=(
  mpv-0.41.0.tar.gz
  https://github.com/mpv-player/mpv/archive/refs/tags/v0.41.0.tar.gz
  ee21092a5ee427353392360929dc64645c54479aefdb5babc5cfbb5fad626209
  17731.patch
  'https://github.com/mpv-player/mpv/commit/75b2ccfeb1ce4ed5a40ac9860fa74f3d1265e13f.patch?full_index=1'
  3906b98b02071a0d5747a400406494ca69cef7afd8d3eee4a99fdbe40dc90c1f
  18383.patch
  'https://github.com/mpv-player/mpv/commit/c5d391adba7bd024954d0df1e0405f5749f4d4ca.patch?full_index=1'
  769b218df220738cc1cf9f81cf696c16518c5dfe56a5ef028e33b22536e0e924
  18679.patch
  https://github.com/mpv-player/mpv/pull/18679.patch
  ec23e46f4f5a5c83215e4b94c529b998e50525d7f00dcb0c14832e98888be55d
)

mkdir -p $dir/dl
for name url sum in $downloads; do
  file=$dir/dl/$name
  [[ -f $file ]] || curl -fsSL -o $file "$url"
  got=$(shasum -a 256 $file | cut -d' ' -f1)
  if [[ $got != $sum ]]; then
    print -u2 "build-libmpv: checksum mismatch for $name (got $got)"
    rm -f $file
    exit 1
  fi
done

# Fresh source tree. `patch`, not `git apply`: the tree sits inside this
# repo, where `git apply` silently skips paths outside the cwd.
rm -rf $dir/src $dir/build $prefix
mkdir -p $dir/src
tar xzf $dir/dl/mpv-0.41.0.tar.gz -C $dir/src --strip-components 1
for p in 17731 18383 18679; do
  patch -d $dir/src -p1 --forward --batch --no-backup-if-mismatch --quiet < $dir/dl/$p.patch
done
# The bug: AudioUnitSetProperty(…, kAudioOutputUnitProperty_ChannelMap, …).
if grep -A1 'AudioUnitSetProperty(p->audio_unit,' $dir/src/audio/out/ao_coreaudio.c | grep -q ChannelMap; then
  print -u2 "build-libmpv: CoreAudio fix not applied"
  exit 1
fi

# Build tools in a private venv (no Homebrew meson/ninja needed).
[[ -x $dir/venv/bin/meson ]] || {
  python3 -m venv $dir/venv
  $dir/venv/bin/pip install --quiet --disable-pip-version-check 'meson==1.12.1' 'ninja==1.13.2'
}
path=($dir/venv/bin $path)

meson setup $dir/build $dir/src --prefix=$prefix --buildtype=release \
  -Dlibmpv=true -Dcplayer=false -Dtests=false -Dbuild-date=false \
  -Dmanpage-build=disabled -Dhtml-build=disabled -Dpdf-build=disabled \
  -Dlua=disabled -Djavascript=disabled -Dlibarchive=disabled -Dlibbluray=disabled \
  -Dvapoursynth=disabled -Drubberband=disabled -Dzimg=disabled -Duchardet=disabled \
  -Dlcms2=disabled -Dvulkan=disabled >/dev/null
meson compile -C $dir/build >/dev/null
meson install -C $dir/build >/dev/null

print -r -- $want > $stamp
print "build-libmpv: installed $(otool -D $prefix/lib/libmpv.2.dylib | tail -1)"
print "build-libmpv: run \`cargo clean -p ttnr-engine\` (or touch crates/ttnr-engine/build.rs) if ttnr was built before"
