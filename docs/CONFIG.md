# Configuration reference

ttnr runs without any configuration. Everything below is optional.

## Files

| File | What |
|---|---|
| `~/.config/ttnr/config.toml` | settings (this page) |
| `~/.config/ttnr/stations.toml` | your stations ([below](#stationstoml)) |
| `~/.config/ttnr/themes/*.toml` | your themes ([THEME-GUIDE.md](THEME-GUIDE.md)) |
| `~/.cache/ttnr/resolve.json` | last good stream URLs (safe to delete) |
| `~/.local/state/ttnr/ttnr.YYYY-MM-DD.log` | log, daily, last 7 kept |
| `$TMPDIR/ttnr-<uid>.sock` | control socket of the running ttnr |

`$XDG_CONFIG_HOME`, `$XDG_CACHE_HOME` and `$XDG_STATE_HOME` replace
`~/.config`, `~/.cache` and `~/.local/state` when set.

A broken `config.toml` or `[keys]` table is reported before the TUI starts;
a broken user theme is skipped and named in the status line.

## config.toml

Plain settings first, then tables (`[keys]`, `[sync_offset]`,
`[sources.<id>]`) – in TOML everything after a table header belongs to that
table. `examples/config.toml` is a commented copy of this page.

| Setting | Values | Default | What |
|---|---|---|---|
| `theme` | theme name | `"plain"` | start theme; `t` cycles, `--theme` overrides |
| `color` | `"auto"`, `"truecolor"`, `"256"`, `"16"`, `"mono"` | `"auto"` | colour depth; auto uses `NO_COLOR`, `COLORTERM`, `TERM` (Terminal.app → 256) |
| `mosaic` | `"auto"`, `"sextant"`, `"quadrant"` | `"auto"` | mosaic glyphs of retro themes; auto = quadrants in Terminal.app, whose fonts lack sextants |
| `effects` | `true`, `false` | `true` | theme animations (boot sequences, tape border, ZX80 blanking) |
| `baud` | bits per second, `0` = instant | `0` | start line speed of terminal themes (`b` cycles 9600/1200/300/110) |
| `media_controls` | `true`, `false` | `true` | macOS Now Playing / Control Center and media keys |
| `output_device` | device name as shown by `d`, or mpv's name | unset | use this device while it is connected, else the system default |
| `exclusive` | `true`, `false` | `false` | exclusive (hog) mode: only ttnr uses the device; macOS moves other apps to another output |
| `match_sample_rate` | `true`, `false` | `false` | switch the device's sample rate to the stream's (system-wide, reverted on stop) |

### `[sync_offset]`

Seconds to delay track information from external providers (BBC's
now-playing API, HLS ID3, `now` of script sources) per source id, so titles
change when the music does. Default 0 (show as soon as known). Adjust live
with `]` / `[`, then note the value here.

```toml
[sync_offset]
bbc = 0
```

### `[diagnostics]`

Optional indicators on one extra line, shown and hidden with `i`
(`diagnostics` key action). All off by default; if none is selected, `i`
shows all of them. Hidden, they cost nothing (no extra redraws); shown,
the line refreshes at most once a second.

| Indicator | Shows |
|---|---|
| `network` | `net active` / `net idle` (data arriving) |
| `download` | read rate (kB/s) and seconds buffered |
| `metadata` | the last HTTP request (metadata polls, resolution): host, age, `ok` / `HTTP n` / `error` |
| `sync_note` | `titles from <source> may lag` when track info comes from an external listing |

```toml
[diagnostics]
download = true
metadata = true
```

### `[keys]`

`action = "key"` or a list of keys. Giving an action replaces its default
keys.

| Action | Default | Action | Default |
|---|---|---|---|
| `toggle_pause` | `space` | `search` | `/` |
| `volume_up` | `+`, `=` | `devices` | `d` |
| `volume_down` | `-` | `cycle_theme` | `t` |
| `mute` | `m` | `help` | `?` |
| `next_station` | `n` | `quit` | `q` |
| `prev_station` | `p` | `sync_later` | `]` |
| `stations` | `s` | `sync_earlier` | `[` |
| `open_url` | `o` | `cycle_baud` | `b` |
| `diagnostics` | `i` | | |

Key names: one character, `space`, `enter`, `tab`, `backspace`, `esc`,
`up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`,
`f1`–`f24`, with optional `ctrl-` / `alt-` prefixes. Fixed and not
remappable: arrows, Enter, Esc (navigation), `j`/`k` in lists, `ctrl-c`
(always quits). A key bound to two actions is an error.

```toml
[keys]
quit = ["q", "ctrl-q"]
next_station = ["n", "right"]
```

### `[sources.<id>]`

An external command that provides stations ([SOURCES.md](SOURCES.md)). The
table name is the source id (not `url`, `stations`, `bbc`, `radio-browser`).

```toml
[sources.somafm]
command = ["~/bin/somafm.py"]   # program and arguments
```

The program path may start with `~/`; a path containing `/` that is not
absolute is relative to the config directory; a bare name is looked up on
`PATH`.

## stations.toml

```toml
[[station]]
id     = "groovesalad"                       # unique
name   = "SomaFM Groove Salad"
source = "url"
url    = "https://somafm.com/groovesalad.pls"

[[station]]
id     = "bbc_6music"                        # BBC service id
name   = "BBC Radio 6 Music"
source = "bbc"

[[station]]
id     = "somafm-groove-salad-128k-aac"
name   = "SomaFM Groove Salad (128k AAC)"
source = "radio-browser"
params = { uuid = "962aa032-0601-11e8-ae97-52543be04c81" }
```

| Field | What |
|---|---|
| `id` | unique, non-empty; `ttnr play <id>` |
| `name` | shown in the list and the view |
| `source` | `url`, `bbc`, `radio-browser`, or a `[sources.<id>]` id |
| `url` | for `url` stations the stream or playlist itself (required); for other sources an optional manual override, tried last |
| `params` | source-specific settings, e.g. `uuid` for radio-browser |

`ttnr search` prints ready entries for radio-browser and script sources.
Stream URLs are resolved when a station starts and cached; a URL that stops
working is re-discovered once automatically (`ttnr refresh` forces it).

## Environment

| Variable | What |
|---|---|
| `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_STATE_HOME` | base directories |
| `TTNR_LOG` | log filter, e.g. `ttnr=debug,info` (default `info`) |
| `NO_COLOR` | monochrome (with `color = "auto"`) |
| `COLORTERM`, `TERM`, `TERM_PROGRAM` | colour depth and mosaic detection |
| `TMPDIR` | directory of the control socket |
