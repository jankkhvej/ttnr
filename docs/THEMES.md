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

- `name`, optional `title` (frame banner, default `ttnr`; empty = none),
  `uppercase` (kept for compatibility; same as `[text] uppercase`).
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

Added in M5 (all optional; `plain.toml` lists the defaults):

- `[screen]` `width`/`height` (logical screen, centred, `0` = full terminal),
  `border` (colour around it), `margin_x`/`margin_y` (paper around the
  content, dropped first on small terminals).
- `[text]` `uppercase`, `ascii_only` (transliterate, else `?`), `map`
  (per-character substitutions, applied last), `numbers` (`decimal`/`octal`
  for volume, sync offset, percentages; the stream format line stays
  decimal). The open-URL input is always shown verbatim.
- `[chrome]` `banner` lines, `prompt`, `cursor` (`block`/`underline`),
  `cursor_blink`, `title` (`plain`/`banner` = two-row bar emulating double
  height; inverse video in mono), `rule` + `rule_fallback` (mosaic line;
  config `mosaic = "auto|sextant|quadrant"` picks the fallback), `printout`
  (append changes as lines like paper instead of redrawing).
- `[status]` templates per state with `{glyph}`, `{message}`, `{percent}`.
- `[text] percent_sign = false` drops `%` (octal themes show `VOL 144`).
- `[effects]` `loading` (`none`/`tape`/`blank`) and `boot` (`none`/
  `zx-spectrum`/`zx80`/`bbc-micro`/`vt100`): a power-on sequence when the
  theme is switched to (and at start; any key skips it) and an animation
  while a station loads. Config `effects = false` turns all of them off;
  the UI ticks (25 fps) only while one runs.
- `[chrome] serial = true` marks a terminal on a serial line
  (`pdp11-teletype`, `pdp11-vt100*`): the `b` key (`cycle_baud`) cycles the
  line speed off → 9600 → 1200 → 300 → 110 baud (start value: config
  `baud`), shown as `9600 BAUD` next to the volume. Printout themes print at
  that speed (old queued lines are printed at once when more than ~4 s would
  be queued); screen themes receive their changed cells one by one, row by
  row (10 bits per character; cursor-addressing bytes not counted). Boot
  sequences run at their own pace; switching a speed on keeps the current
  screen, only later changes arrive slowly; at start the screen is typed
  out from blank.

Real DECDHL double height is not used: ratatui cannot emit the line
attribute escapes, so double height is emulated by two rows.

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

### `zx-spectrum` – Sinclair ZX Spectrum 48K (researched 2026-11-03)
Facts (BASIC manual, 48K ROM disassembly, World of Spectrum FAQ): white
paper, black ink, white border after power-on; upper and lower case; `` ` ``
shows as £, `^` as ↑, 127 as ©; 2 × 2 block graphics; reports `code text`
at the bottom (`0 OK`, `D BREAK - CONT repeats`, `R Tape loading error` …).
Tape loading (ROM LD-BYTES): border red while searching, red/cyan stripes
during the pilot tone, blue/yellow during data; pilot pulse 2168 T-states
(≈ 9.7 scan lines at 224 T-states per line), bits 855/1710 T-states per
pulse. Power-on (START_NEW): border white, RAM filled with 2 and read back,
then CLS and `© 1982 Sinclair Research Ltd` on the bottom line.
Design: 32-column screen on a white border, blue double-height station bar,
status as reports (`LOAD ""` while loading, `0 OK, 0:1` playing, `D BREAK -
CONT repeats, 0:1` paused, `8 End of file` ended); tape stripes in the
border while a station loads and ~2 s after the audio starts (half-block
`▀` = two stripes per row), kept 2 columns / 1 row away from the text by a
paper margin (owner, design choice); boot (~1.4 s): static garbage, black
screen with thin red lines `¦` (RAM test – red on black inferred from the
bytes the ROM writes; owner remembers them as lines/exclamation marks),
then the copyright line.
UNVERIFIED: the `, line:statement` suffix wording; exact RGB (0xD7 level
used); the garbage/red-line look (forum accounts, consistent with the ROM).
The "exclamation marks" pattern some remember was not found in sources.

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

### Boot sequences of the other themes
- `zx80`: blank picture, inverse `K` cursor bottom left (Operating Manual);
  while a station loads the picture blanks, as the ZX80 did while busy.
- `bbc-mode7`: `BBC Computer 32K`, `BASIC`, `>` with a flashing cursor.
- `pdp11-vt100*`: VT100 self-test (test data, erase, cursor top left), then
  RSX-11M console lines `RSX-11M V4.6 BL56 128.K MAPPED`, `>RED DU:=SY:`,
  `>RED DU:=LB:`, `>MOU DU:RSXM56`, `>@DU:[1,2]STARTUP` (SIMH logs).
- `pdp11-teletype`: none (the banner is printed, at `baud` speed if set).

## Proposals (not scheduled)

- **Teletype: everything on paper** (owner, 2026-11-03: keep as proposal).
  Help, station list, devices and the URL prompt would be printed as lines
  instead of popups: `?` prints the key table; `s`/`d` print a numbered list
  and a `STATION? ` prompt answered by number + Enter; `o` prints `.` and
  echoes the typed URL. Core state unchanged; work in the TUI printout
  renderer and input handling (medium).

## Acceptance for every theme

- Snapshot tests at 32×16, 40×25 and 100×30 (or the theme's natural size).
- Readable at all three; nothing clipped silently (truncate with the
  theme's own ellipsis glyph).
- Works in 16-colour and monochrome fallback.
- A short "why it looks like this" note with sources in RESEARCH.md.
