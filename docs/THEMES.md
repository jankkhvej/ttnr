# Themes — design spec

Themes recreate the *feel* of historic machines **in text only**, as closely
as a modern terminal and monospace font allow. They must stay readable and
show the same information as `plain`.

> The machine details below were researched on 2026-11-03 (sources in
> RESEARCH.md, "Retro themes"). Anything not confirmed is marked `UNVERIFIED`.

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

### `pdp11-vt100` — DEC PDP-11 seen through a DEC VT100 (researched 2026-11-03)
Facts (sources in RESEARCH.md, "Retro themes"):
- VT100: 80 × 24 characters (132 × 14, or 132 × 24 with the AVO); 12″ CRT
  with **P4 (white) phosphor**; 7 × 9 dot-matrix characters. Green/amber
  tubes belong to later terminals (VT220) → offered only as palette variants.
- Attributes: bold, underscore, blink, reverse (SGR 1/4/5/7); reverse screen
  (DECSCNM) = dark characters on white.
- DEC Special Graphics line drawing: corners ┌┐└┘, tees ├┤┬┴, ┼, ─ │,
  scan lines ⎺⎻─⎼⎽, ◆ ▒ ° ± ≤ ≥ π ≠ £ ·.
- Double-height (DECDHL, `ESC # 3` top / `ESC # 4` bottom, same text on two
  adjacent lines) and double-width (DECDWL `ESC # 6`) lines.
- RT-11 monitor prompt `.`; KMON errors `?KMON-F-<text>`; dates `dd-mmm-yy`
  (`.DATE 18-MAY-91`). RSX-11M: implicit prompt `>`, explicit `MCR>`
  (CTRL/C); banner like `RSX-11M V4.6 BL56`. DEC culture: octal numbers.
Design: white-on-black (P4) palette, `amber`/`green` variants; box borders
drawn from the Special Graphics repertoire; 80-column logical screen;
`MCR>` prompt for open URL; numbers (volume, bitrate) in octal with a
trailing `(8)`-free DEC style (e.g. `VOL 144`); errors as `?TTNR-F-…`;
title line double-height where the terminal supports DECDHL.
UNVERIFIED: exact RT-11 V5.x banner string; MCR error texts.

### `pdp11-teletype` — Teletype Model 33 ASR printout (researched 2026-11-03)
Facts: prints **upper case only** (64-character ASCII subset), **72
characters per line**, 10 characters per second (110 baud); 1963 ASCII had
↑ and ← where 1967 ASCII has `^` and `_` (the keyboard showed the arrows).
Design: uppercase, ASCII-only, no borders, 72-column logical screen, dark
ink on paper-white; new information is *printed* (appended lines, the view
scrolls like paper) instead of redrawn; `^`→`↑`, `_`→`←` (design choice).
UNVERIFIED: paper/ink colours, exact glyphs an ASR-33 printed for ^ and _.

### `zx80` — Sinclair ZX80 (researched 2026-11-03)
Facts (ZX80 Operating Manual): 32 × 24 character screen, **black on white**
(inverse = white on black); no colour, **no lower case**; 64 glyphs + their
inverse versions; 2 × 2 block graphics and a 50 % grey (chequered) glyph;
listing at the top, edit line at the bottom; inverse `K`/`L` cursor (keyword
/ letter mode), inverse `S` marks a syntax error; the screen goes blank while
the machine computes (no FAST/SLOW on the ZX80); error reports `n/m` (error
number / line), codes 0–9 (0 = OK or BREAK, 2 = variable not found, 4/5 =
no room, 6 = overflow, 9 = STOP …).
Design: 32-column logical screen centred on a white border; uppercase;
quadrant blocks ▖▗▘▝▌▐▀▄█ and ▒; inverse video for emphasis and the cursor;
status as a report code line (`0/0` playing, `2/0` station not found,
`4/0` network error …). Optional, off by default: blank flash on station
change (respects reduced motion).
UNVERIFIED: exact emulator colours (pure #000/#FFF chosen).

### `bbc-mode7` — BBC Micro (Acorn) MODE 7 teletext (researched 2026-11-03)
Facts: 40 × 25 SAA5050 teletext; colours black, red, green, yellow, blue,
magenta, cyan, white (3-bit RGB); control codes occupy a cell (shown as a
space); double height = top halves on one row, bottom halves on the next;
flash; contiguous/separated 2 × 3 mosaics; English character set shows `#`
as `£`, `[`←, `\`½, `]`→, `^`↑, `_`#, `` ` ``—, `{`¼, `|`‖, `}`¾, `~`÷.
Banner after power-on: `BBC Computer 32K`, then `BASIC`, prompt `>`
(`Acorn DFS` only with discs); cursor flashes at the bottom of the cell.
Unicode sextants U+1FB00–U+1FB3B (Unicode 13; the 4 patterns equal to space,
▌, ▐, █ are not encoded); separated mosaics U+1CC00 block (Unicode 16).
Menlo/SF Mono lack sextants (kitty, Ghostty, WezTerm draw them themselves).
Design: 40-column screen on black; station name as a double-height coloured
banner (DECDHL where supported, else two rows); teletext header line; boot
banner + `>` prompt; mosaics as sextants with a quadrant fallback (`mosaic =
"auto|sextant|quadrant"`, auto picks quadrants in Terminal.app).
UNVERIFIED: blank-line layout of the banner; exact RGB (ideal 0/255 used);
start line of the cursor (underline assumed).

## Acceptance for every theme

- Snapshot tests at 32×16, 40×25 and 100×30 (or the theme's natural size).
- Readable at all three; nothing clipped silently (truncate with the
  theme's own ellipsis glyph).
- Works in 16-colour and monochrome fallback.
- A short "why it looks like this" note with sources in RESEARCH.md.
