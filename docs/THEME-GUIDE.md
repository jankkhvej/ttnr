# Writing a theme

A theme is one TOML file: colours, characters, case, layout width, banner,
prompt, status messages and optional animations – text only, never images.
ttnr loads your themes from `~/.config/ttnr/themes/*.toml`; `t` cycles
through built-ins and yours. The full list of keys is in
[THEMES.md – Theme files](THEMES.md#theme-files-schema-tutorial-theme-guidemd).

## 1. Start from `plain`

```zsh
mkdir -p ~/.config/ttnr/themes
cp crates/ttnr-themes/themes/plain.toml ~/.config/ttnr/themes/amber.toml
```

Give it its own name (a theme named like a built-in replaces it):

```toml
name  = "amber"
title = "ttnr"        # text in the frame; "" for none
```

Run it with `ttnr --theme amber`. Mistakes (unknown keys, bad colours) are
reported in the status line with the file name and the error, and the
theme is skipped; ttnr itself keeps running.

## 2. Colours

Six roles: `fg` (text), `bg` (background), `accent` (station name,
highlights, title bar), `dim` (secondary text, borders, footer), `warn`,
`error`. Values: `"default"` (the terminal's own colour), `"faint"`
(terminal colour, drawn faint – works with any colour scheme), ANSI names
(`black` … `bright_white`), `"#rrggbb"`, or a 256-colour index.

```toml
[palette]
fg     = "#ffb000"
bg     = "#000000"
accent = "#ffc840"
dim    = "#a06a00"
warn   = "#ffc840"
error  = "#ff6040"

[palette_16]          # optional: exact choices for 16-colour terminals
fg     = "yellow"
accent = "bright_yellow"
```

Colours are downgraded automatically to what the terminal can show
(truecolor → 256 → 16 → none). Check each depth by setting
`color = "256"`, `"16"` or `"mono"` in `config.toml`. In mono only bold,
reverse and faint remain – make sure the theme still reads.

## 3. Characters

```toml
[glyphs]
borders   = "ascii"   # "box", "double", "ascii", "none", or six glyphs:
# borders = { top_left = "+", top_right = "+", bottom_left = "+",
#             bottom_right = "+", horizontal = "-", vertical = "|" }
ellipsis  = "..."     # end of cut-off text
dash      = " - "     # between artist and title
separator = "  "      # between items of a line
playing   = ">"       # status glyphs
paused    = "="
busy      = "*"
stopped   = "-"
error     = "!"
current   = "*"       # marks the playing station in lists
cursor    = ">"       # the highlighted row
```

Stick to characters your machine had: the built-in retro themes use only
glyphs from their real character sets.

## 4. Text rules

```toml
[text]
uppercase    = true        # no lower case (teletypes, ZX80)
ascii_only   = true        # Żółw → Zolw; characters without a look-alike → ?
map          = { "#" = "£" }   # per-character substitutions, applied last
numbers      = "octal"     # volume, sync offset, percentages (DEC style)
percent_sign = false       # "VOL 144" instead of "VOL 144%"
```

Typed URLs are always shown as typed.

## 5. Screen size and chrome

Old machines had fixed screens. A logical screen is centred, the rest is
filled with the border colour:

```toml
[screen]
width    = 40          # columns; 0 = the whole terminal
height   = 25
border   = "#000000"
margin_x = 0           # paper around the text (dropped first when small)
margin_y = 0

[chrome]
banner       = ["MY COMPUTER 64K", ""]  # lines on top
prompt       = "READY> "                # the open-URL prompt
cursor       = "block"                  # or "underline"
cursor_blink = true
title        = "banner"   # station as a two-row bar (double height)
rule         = "▄"        # glyph repeated under the title
rule_fallback = ""        # used with config mosaic = "quadrant"
printout     = false      # print lines like a teletype instead of redrawing
serial       = false      # a serial terminal: `b` simulates line speed
```

## 6. Status messages

Templates per state; `{glyph}` is the status glyph, `{message}` the error
or reason, `{percent}` the buffer level (" 40%").

```toml
[status]
idle         = "READY"
resolving    = "SEARCHING..."
loading      = "LOADING..."
buffering    = "LOADING{percent}"
playing      = "{glyph} PLAYING"
paused       = "BREAK"
reconnecting = "?LINE {message}"
ended        = "END"
error        = "?ERROR {message}"
```

## 7. Animations (optional)

```toml
[effects]
boot      = "vt100"      # "none", "zx-spectrum", "zx80", "bbc-micro", "vt100"
loading   = "none"       # "tape" (ZX Spectrum border), "blank" (ZX80)
boot_text = ["MY OS V1.0", "READY"]   # lines the vt100 boot prints
```

Boot sequences play when the theme is switched to; any key skips them.
`effects = false` in `config.toml` turns all animations off.

## 8. Check it

- Sizes: the UI must stay readable from 32×16 up; resize the terminal.
- Depths: `color = "16"` and `"mono"`.
- Fonts: try Terminal.app's default font (it lacks some block glyphs).
- To ship a theme with ttnr: put the file in `crates/ttnr-themes/themes/`,
  add it to `BUILTIN` in `crates/ttnr-themes/src/lib.rs`, run
  `INSTA_UPDATE=always cargo test -p ttnr-tui`, review the new snapshots,
  and log the facts it relies on (with sources) in `docs/RESEARCH.md`.
