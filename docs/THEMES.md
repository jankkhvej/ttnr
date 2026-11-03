# Themes — design spec

Themes recreate the *feel* of historic machines **in text only**, as closely
as a modern terminal and monospace font allow. They must stay readable and
show the same information as `plain`.

> ⚠️ The historical details below are the owner's/assistant's starting notes,
> **not yet verified**. Before implementing each theme, research it
> (manuals, emulators, photos, museum/archive sources), log findings with
> URLs in RESEARCH.md, and correct this file. Mark anything you could not
> confirm as `UNVERIFIED`.

## What a theme can control

| Aspect | Examples |
|---|---|
| Palette | fg/bg, accent, dim, warning; per depth (24-bit, 256, 16, mono) |
| Case | as-is · UPPERCASE-only (map lowercase → uppercase) |
| Glyph set | full Unicode · box-drawing only · ASCII only · block mosaics |
| Borders | none · single/double line · `+-|` ASCII · solid blocks |
| Logical screen | width in columns (e.g. 32, 40, 80), centred, with "screen border" fill |
| Banner | boot/greeting text shown on start and in help |
| Prompt | e.g. `.`, `>`, `MCR>` for the open-URL input line |
| Cursor | block/underline/inverse, blink on/off |
| Numbers | decimal · octal (with or without leading `0`) |
| Status style | how errors/reports look (e.g. `?ERROR`, `0/0`-style report codes) |
| Effects | bold, reverse, blink, simulated double-height (two rows) |

Fonts: themes must work with any monospace font. README may *suggest*
open-licence retro fonts per theme; never require them.

## Theme files (implemented in M3)

A theme is one TOML file; built-ins are embedded from
`crates/ttnr-themes/themes/*.toml`, user themes live in
`~/.config/ttnr/themes/*.toml` (a user theme with a built-in's `name`
replaces it). Start from `crates/ttnr-themes/themes/plain.toml`.

- `name`, optional `title` (frame banner, default `ttnr`), `uppercase`.
- `[palette]` roles `fg`, `bg`, `accent`, `dim`, `warn`, `error`: `"default"`,
  ANSI names (`black` … `bright_white`), `"#rrggbb"`, or `0`–`255`.
  Optional `[palette_256]` / `[palette_16]` override single roles at those
  depths; otherwise colours are downgraded automatically (truecolor → nearest
  xterm-256 cube/grey → nearest of 16 ANSI → terminal default in mono).
- `[glyphs]`: `borders` (`"box"`, `"double"`, `"ascii"`, `"none"`, or a table
  `top_left`/`top_right`/`bottom_left`/`bottom_right`/`horizontal`/`vertical`),
  `ellipsis`, `dash`, `separator`, status glyphs `playing`/`paused`/`busy`/
  `stopped`/`error`, list marks `current`/`cursor`.
- Unknown keys are errors; a broken user theme is skipped and reported.
- Colour depth: config `color = "auto|truecolor|256|16|mono"`; auto uses
  `NO_COLOR`, `COLORTERM`, `TERM` (Terminal.app is treated as 256).

The remaining controls listed above (logical width, prompt, cursor, number
format, effects, banner) are added with the retro themes in M5.

## Built-in themes

### `plain` (default)
Clean, modern, uses the terminal's own colours, box-drawing borders.

### `pdp11-vt100` — DEC PDP-11 seen through a DEC video terminal
Starting notes (verify): PDP-11s had no built-in display; operators used
terminals such as DEC VT05/VT52/VT100 and printing Teletypes. Idea:
monochrome phosphor look (white/green/amber variants as palette options),
80 columns, DEC Special Graphics line-drawing borders, bold/reverse for
emphasis, RT-11-style `.` monitor prompt or RSX-11 `MCR>` for the URL
input, numbers shown in **octal** (DEC convention) with an option to turn
off. Optional text "front panel" line of lamps (`●○`) showing e.g. volume
or seconds as bits.

### `pdp11-teletype` — ASR-33 style printout
Starting notes (verify): uppercase-only, no colour (dark text on
paper-white background), ASCII only, no borders, new info "printed" as
appended lines rather than redrawn in place (scrolling log feel), 72-column
line width.

### `zx80` — Sinclair ZX80
Starting notes (verify): black characters on a white screen with white
border, 32×24 character screen, **uppercase only**, inverse-video
characters for emphasis/cursor, 2×2 block mosaic graphics (Unicode
quadrant blocks `▖▗▘▝▌▐▀▄█`), terse numeric error/report codes at the
bottom line. Optional, off-by-default "FAST mode" screen blank flash on
station change (must be disable-able, respect reduced-motion).

### `bbc-mode7` — BBC Microcomputer (Acorn), MODE 7 teletext
Starting notes (verify): 40×25 teletext screen, black background, the
seven teletext colours (red, green, yellow, blue, magenta, cyan, white),
**double-height** headings (emulate with two rows), 2×3 mosaic graphics
(Unicode "Symbols for Legacy Computing" sextants U+1FB00–U+1FB3B with a
quadrant-block fallback when the font lacks them), boot banner in the
style of `BBC Computer 32K` / `BASIC` and a `>` prompt with a flashing
cursor. Station name as a coloured double-height title bar, like teletext
pages.

## Acceptance for every theme

- Snapshot tests at 32×16, 40×25 and 100×30 (or the theme's natural size).
- Readable at all three; nothing clipped silently (truncate with the
  theme's own ellipsis glyph).
- Works in 16-colour and monochrome fallback.
- A short "why it looks like this" note with sources in RESEARCH.md.
