---
name: crablink web UI
description: A gate board that issues boarding passes; every Link is a pass from its Domain to its Target.
world: "vernacular-ephemera-boarding-pass-and-gate-board"
colors:
  crab-red: "#e0412a"
  crab-red-fill: "#d13b24"
  crab-red-press: "#b5311d"
  shell-ink: "#17181c"
  paper: "#ffffff"
  paper-2: "#f6f7f8"
  panel: "#e8eaed"
  muted: "#565b64"
  hairline: "#d9dce0"
  hairline-pass: "#e1e3e7"
  line-strong: "#c3c7cd"
  field-line: "#868c96"
  danger: "#b42318"
  ok: "#13622b"
  warn: "#8a5300"
  band-muted: "#b8bcc2"
  band-edge: "#2a2c33"
  flap-leaf-top: "#2c2e35"
  flap-leaf-bottom: "#24262c"
  flap-seam: "#0b0b0d"
  dark-band: "#0e0f12"
  dark-surface: "#1e1f24"
  dark-surface-2: "#24262c"
  dark-text: "#edeef0"
  dark-muted: "#a3a8b1"
  dark-line: "#2f3137"
  dark-line-strong: "#3d4048"
  dark-field-line: "#6f7580"
  dark-band-edge: "#464a53"
  dark-danger: "#ff8a7a"
  dark-ok: "#5cc97c"
  dark-warn: "#f2b451"
typography:
  display:
    fontFamily: "'JetBrains Mono Variable', 'JetBrains Mono', ui-monospace, Menlo, Consolas, 'PingFang SC', 'Microsoft YaHei', monospace"
    fontSize: "clamp(2.25rem, 9cqi, 3.25rem)"
    fontWeight: 650
    lineHeight: 1.05
    letterSpacing: "-0.02em"
  headline:
    fontFamily: "'Inter Variable', Inter, system-ui, -apple-system, 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', sans-serif"
    fontSize: "1.375rem"
    fontWeight: 650
    lineHeight: 1.2
    letterSpacing: "-0.015em"
  title:
    fontFamily: "'Inter Variable', Inter, system-ui, -apple-system, 'PingFang SC', 'Microsoft YaHei', sans-serif"
    fontSize: "1rem"
    fontWeight: 600
    lineHeight: 1.3
    letterSpacing: "-0.005em"
  body:
    fontFamily: "'Inter Variable', Inter, system-ui, -apple-system, 'PingFang SC', 'Microsoft YaHei', sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 400
    lineHeight: 1.5
  data:
    fontFamily: "'JetBrains Mono Variable', 'JetBrains Mono', ui-monospace, Menlo, monospace"
    fontSize: "0.875rem"
    fontWeight: 500
    lineHeight: 1.4
    fontFeature: "'tnum' 1"
  figure:
    fontFamily: "'JetBrains Mono Variable', 'JetBrains Mono', ui-monospace, Menlo, monospace"
    fontSize: "1.25rem"
    fontWeight: 600
    lineHeight: 1.1
    fontFeature: "'tnum' 1"
  label:
    fontFamily: "'JetBrains Mono Variable', 'JetBrains Mono', ui-monospace, Menlo, 'PingFang SC', 'Microsoft YaHei', monospace"
    fontSize: "0.6875rem"
    fontWeight: 500
    lineHeight: 1.2
    letterSpacing: "0.08em"
rounded:
  stamp: "3px"
  tile: "4px"
  control: "6px"
  sheet: "8px"
  menu: "10px"
  pass: "14px"
  pill: "999px"
spacing:
  2xs: "0.25rem"
  xs: "0.375rem"
  sm: "0.5rem"
  md: "0.75rem"
  lg: "1rem"
  xl: "1.25rem"
  2xl: "1.5rem"
  3xl: "1.75rem"
  4xl: "2rem"
components:
  button-accent:
    backgroundColor: "{colors.crab-red-fill}"
    textColor: "{colors.paper}"
    rounded: "{rounded.control}"
    padding: "0 1rem"
    height: "2.5rem"
  button-accent-hover:
    backgroundColor: "{colors.crab-red-press}"
  button-ink:
    backgroundColor: "{colors.shell-ink}"
    textColor: "{colors.paper}"
    rounded: "{rounded.control}"
    padding: "0 1rem"
    height: "2.5rem"
  button-default:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.shell-ink}"
    rounded: "{rounded.control}"
    padding: "0 1rem"
    height: "2.5rem"
  button-small:
    rounded: "{rounded.control}"
    padding: "0 0.75rem"
    height: "2.25rem"
  button-danger:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.danger}"
    rounded: "{rounded.control}"
    height: "2.5rem"
  input:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.shell-ink}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: "0 0.75rem"
    height: "2.5rem"
  input-pass:
    height: "2.75rem"
    rounded: "{rounded.control}"
  tag:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.shell-ink}"
    rounded: "{rounded.pill}"
    padding: "0.0625rem 0.5rem"
  stamp:
    textColor: "{colors.shell-ink}"
    typography: "{typography.label}"
    rounded: "{rounded.stamp}"
    padding: "0.1875rem 0.4375rem"
  band:
    backgroundColor: "{colors.shell-ink}"
    textColor: "{colors.paper}"
    height: "3.75rem"
    padding: "0 2rem"
  board-row-selected:
    backgroundColor: "{colors.shell-ink}"
    textColor: "{colors.paper}"
  pass-card:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.shell-ink}"
    rounded: "{rounded.pass}"
    padding: "1.5rem 1.5rem 1.25rem 1.75rem"
  pass-stub:
    backgroundColor: "{colors.panel}"
    width: "10.5rem"
    padding: "1.5rem 1rem 1.25rem"
  qr-block:
    backgroundColor: "{colors.paper}"
    rounded: "{rounded.control}"
    width: "7.75rem"
    padding: "0.375rem"
  ink-board:
    backgroundColor: "{colors.shell-ink}"
    textColor: "{colors.dark-text}"
    rounded: "{rounded.pass}"
    padding: "1.125rem 1.5rem 1.5rem"
  flap-tile:
    backgroundColor: "{colors.flap-leaf-top}"
    textColor: "{colors.paper}"
    typography: "{typography.figure}"
    rounded: "{rounded.tile}"
    height: "2.75rem"
  toast:
    backgroundColor: "{colors.shell-ink}"
    textColor: "{colors.paper}"
    rounded: "{rounded.sheet}"
    padding: "0.75rem 1.125rem"
  menu:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.shell-ink}"
    rounded: "{rounded.menu}"
    padding: "0.375rem"
---

# Design System: crablink web UI

## Overview

**Creative North Star: "The Gate Board That Issues Passes"**

Every Link is a boarding pass, and the Links page is the departures board that issues them. The board is a mono, tabular list on the open ground: Short URL as the row's headline, Target and Tags beneath, Click Count flush right. Picking a row inverts it to Shell Ink and lays its pass on the counter beside the board: a white, notched card whose route runs from the Code (at display size) across a dashed flight line with a plane to the Target's host, segments of mono data under hairlines, a perforated tear line, and a Panel-grey stub carrying the QR scan block, the Short URL and its Copy and Download QR. Under the pass sits a Shell Ink split-flap board of the Link's last 7 days of Clicks. Clicks, Manage Workspace, Domains, Settings, Instance Admin, sign-in and invites reuse the same grammar: a page is either a gate board (rows under a 2px ink rule) or a pass/ticket on the ground (card, perforation, stub or tear-off foot).

The system is calm, ink-on-paper and operational. Density is moderate: 15px Inter body, mono micro-labels and figures, hairline dividers, almost no fills. Texture comes only from the world's own materials: hairline segment rules, the dashed perforation with its punched notches, the dashed flight path, the QR block and the flap tiles' seam. Crab Red appears on one thing at a time, the primary action or the fresh-Link mark. The header band is Shell Ink in both themes; in dark theme the ground goes ink and the pass stays light paper, like a printed ticket on a dark counter.

Provenance: the world is the catalog world `vernacular-ephemera-boarding-pass-and-gate-board`, dealt as a challenger. The surface brief (`.impeccable/surfaces/web-src-links-tsx.md`) records `seed:5255701d1f7e8592`, but `impeccable concept-seed --check-seed` reports SEED UNKNOWN on this machine and no roll output from the build was kept, so the seed is unverifiable; this was disclosed at the 2026-11-19 finish review.

**Key Characteristics:**
- A Link is a pass: route (Code to Target host), segments, perforation, stub with QR.
- Lists are gate boards: mono rows, tabular figures, a 2px ink rule under the heads, hairlines between rows, the selected row inverted.
- Crab Red is scarce: the primary action fill and the fresh mark only.
- The pass is paper in both themes; the band and the 7-day board are ink in both themes.
- Motion is airport grammar: print, fly, tear, flap, draw, rerank; each move at most 400ms, ease-out, never looping, off under reduced motion.

## Colors

Ink on paper with one red: Shell Ink and a cool grey ramp carry everything, Crab Red is a signal.

### Primary
- **Crab Red** (`crab-red`): the brand red. Marks a fresh Link (the New/Updated stamp's border and the faint 6% row tint) and is the base of the primary action. Never small body text.
- **Crab Red Fill** (`crab-red-fill`): Crab Red one step deeper, the actual fill of the accent button (New Link, Create) so white text reaches 4.5:1. **Crab Red Press** (`crab-red-press`) is its hover.

### Neutral
- **Shell Ink** (`shell-ink`): text, the header band, the inverted selected row, the ink primary button, the 2px board rules, toasts, the focus ring in light theme, the plane.
- **Paper** (`paper`) and **Paper Two** (`paper-2`): ground and surfaces in light theme; Paper is the pass and ticket in both themes. Paper Two is row hover, read-only fields and dialog feet.
- **Panel** (`panel`): the stub's material, apart from the pass's white so the tear line separates two surfaces; also Tag chips, the avatar disc, and the selected row in dark theme.
- **Slate Muted** (`muted`): secondary text, micro-labels, hosts before a Code.
- **Hairline** (`hairline`, `hairline-pass` on the pass), **Strong Line** (`line-strong`): row and segment dividers; card borders, perforation and flight dashes.
- **Field Line** (`field-line`): every control's 1px border, at 3:1.
- **Band Muted** (`band-muted`) and **Band Edge** (`band-edge`): inactive tab text and the band's bottom edge.
- **Flap leaves and seam** (`flap-leaf-top`, `flap-leaf-bottom`, `flap-seam`): the split-flap tile's gradient and its 1px dark seam.
- **Status** (`danger`, `ok`, `warn`): alerts, confirmations and state stamps/lamps; text-only, never fills.
- **Dark theme** (`dark-*`): the ground becomes Shell Ink, the band `dark-band`, surfaces `dark-surface` / `dark-surface-2`, text `dark-text`; the band and tab bar get the firmer `dark-band-edge` so the band does not merge with the ground.

### Named Rules
**The One Red Rule.** Crab Red fills only the primary action and marks only a fresh Link. While a blank pass is open, its New Link button goes neutral and pressed so the stub's Create is the only red on screen.

**The Paper Pass Rule.** The pass, tickets and confirm dialogs re-declare the light tokens and stay paper in dark theme; the band and the 7-day board stay ink in light theme. Theme changes the ground, never the materials.

**The Inversion Rule.** Selection is shown by inverting the row (Shell Ink with white text in light theme, Panel with ink text in dark), never by a colored highlight.

## Typography

**Display / Data Font:** JetBrains Mono Variable (self-hosted via @fontsource-variable, with ui-monospace and CJK fallbacks)
**Body Font:** Inter Variable (self-hosted via @fontsource-variable, with system-ui, PingFang SC, Microsoft YaHei)
**Label Font:** JetBrains Mono, uppercase

**Character:** Inter speaks; JetBrains Mono prints. Anything a person would read off a ticket or a board (Codes, Short URLs, hosts, counts, dates, field keys) is mono with tabular figures; prose, buttons and headings are Inter. Contextual ligatures are off everywhere so `://` reads as typed.

### Hierarchy
- **Display** (mono 650, clamp(2.25rem, 9cqi, 3.25rem), 1.05, -0.02em): the Code on the pass, the Click total on the Clicks pass, a Workspace name. A long Code shrinks to fit the pass's main column down to 1.25rem, then breaks after a hyphen or underscore.
- **Headline** (Inter 650, 1.375rem, 1.2, -0.015em, balanced): page titles.
- **Title** (Inter 600, 1rem, 1.3): section and board titles.
- **Body** (Inter 400, 0.9375rem, 1.5): prose, inputs; ledes capped at 65ch.
- **Data** (mono 500, 0.875rem, 1.4, tabular): segment values, board rows (0.875rem/1.35), hosts (0.9375rem), Click Counts (0.9375rem).
- **Figure** (mono 600, 1.25rem, 1.1, tabular): split-flap tiles.
- **Label** (mono 500, 0.6875rem, 0.08em, uppercase): field keys, column heads, segment keys, stamps (600). In Chinese it steps up to 0.75rem at 0.04em; CJK falls through the mono stack to PingFang or YaHei.

### Named Rules
**The Printed Data Rule.** If it could be printed on a pass or flipped on a board, it is mono with tabular figures; if it is a sentence or an action, it is Inter.

**The Field Key Rule.** A micro-label names the field or column directly beneath it (FROM, TARGET, EXPIRY, CLICKS, WORKSPACE over its name). It is never a decorative line above a heading or a paragraph.

## Layout

The Links page is full-bleed and split: the board column on the left (`minmax(0, 1fr)`, padding 1.75rem/2rem, hairline right border) and the pane on the right (`minmax(28rem, 42%)`), sticky under the 3.75rem band, its pass top-aligned so editing grows it downward and nothing moves under the pointer. Other pages sit in `main` (max 76rem, padding 2rem) or on a `counter` (max 48rem) that holds one pass. Instance Admin is a `desk`: boards beside a 22rem sticky invite ticket. Sections on the ground are separated by a hairline and 2rem, stacked boards by 3rem of space.

Spacing runs on a rem scale (0.25 to 2rem, see `spacing`), with 0.75rem the default gap and cell padding (0.875rem in board rows).

Responsive behavior:
- **Phone, below 48rem:** the band drops its tabs for a fixed bottom tab bar (Links / Clicks / Settings, 3.25rem, ink, a 3px white top rule on the current tab); filters go full width; gate-board rows become stacked line cards; toasts lift above the tab bar.
- **Touch or phone:** every control is at least 2.75rem (44px) high and icon buttons 2.75rem wide.
- **Below 60rem:** the Links page is one column, the pass opens above the board, the board head sticks with New Link in reach, and board rows become two-line grids.
- **WIDE, 60rem and up:** board and pane side by side; the band is sticky.
- **Below 64rem:** the desk stacks; the band tightens to 1.25rem padding.
- **ROOMY, 90rem and up:** board rows show Copy and QR beside their More menu; below it they fold into the menu.
- **Container query, pass at 30rem or narrower:** the pass stacks, the stub moves under the main column, the perforation turns horizontal and the main column and stub each punch their half of its notches, and field segments go one per line.

## Elevation & Depth

Mostly flat: the ground, boards and rows have no shadow, and depth is tonal (Paper over Panel, ink band over paper). One soft, ambient lift is reserved for objects that are physically on top: the pass, tickets, menus and dialogs. The pass and tickets, whose notches are punched through them by a mask, take the same shadow as a `drop-shadow` filter so it follows the holes.

### Shadow Vocabulary
- **Pass lift** (`box-shadow: 0 1px 2px rgb(23 24 28 / 0.06), 0 12px 28px -10px rgb(23 24 28 / 0.18)`; dark `0 1px 2px rgb(0 0 0 / 0.4), 0 16px 36px -12px rgb(0 0 0 / 0.7)`): tickets, menus, confirm dialogs.
- **Pass drop** (`filter: drop-shadow(0 1px 1px rgb(23 24 28 / 0.06)) drop-shadow(0 8px 12px rgb(23 24 28 / 0.1))`): the notched pass card and tickets.
- **Toast** (`box-shadow: 0 12px 28px -10px rgb(0 0 0 / 0.45)`).
- **Pressed** (`box-shadow: inset 0 1px 2px rgb(23 24 28 / 0.18)`): New Link while its blank pass is open.
- **Rule underlines** (`inset 0 -3px 0` on band tabs, `inset 0 3px 0` on phone tabs): current-tab marks, drawn as inset rules, not elevation.

### Named Rules
**The On-the-Counter Rule.** Only things lying on the counter (pass, ticket, menu, dialog, toast) cast a shadow, and it is soft and ambient. Rows, boards and sections never lift.

## Shapes

Gently rounded and paper-like. The pass and tickets are 14px cards; controls are 6px; sheets and toasts 8px; menus 10px; flap tiles 4px; stamps 3px; Tag chips are pills; the avatar is a disc. The pass's signature silhouette is the perforation: a 2px dashed line, the stub's left border, 10.5rem from the right edge, with 11px half-circle notches punched through the card at top and bottom (the main column and the stub each draw their own side of the card and mask their half of each notch, so the shadow follows and the stub can be torn off). Tickets, the Instance Admin coupon and confirm dialogs use the same idea horizontally: a dashed top rule on a Paper Two foot with half-circle notches at its ends; on the pass and tickets they are punched (the body and the foot each mask their half), so the ground and the shadow show through, while confirm dialogs have none. The flight path is a 1.5px dashed rule; the loading rule a 2px dashed one (6px dash, 5px gap). Dashed 1.5px borders also mark placeholders and secrets (blank QR, empty pane, revealed API Key).

## Components

### Buttons
Firm, quiet and exact.
- **Shape:** gently rounded (6px), 2.5rem high, 0 1rem padding, Inter 600 0.875rem; small 2.25rem / 0.75rem / 0.8125rem; row actions 2rem / 0.75rem.
- **Accent:** Crab Red Fill with white text; New Link and Create only (see The One Red Rule).
- **Ink:** Shell Ink with paper text; the main action where red is not warranted. Hover mixes 14% toward the on-color.
- **Default:** surface fill, Field Line border; hover darkens the border to the text color.
- **Danger:** danger text and a 70% danger border; hover tints 8%.
- **States:** press nudges down 1px; disabled is 55% opacity; focus is a 2px ring in the focus color, 2px offset.
- **Copy:** Copy and Copied share one grid cell, the idle label hidden, so the button never changes width; Copied flies a small plane in.

### Chips and Stamps
- **Tag:** Panel pill, Inter 500, 1.125rem line; as wide as its name up to 9rem on a board row, then ellipsised, with a `+N` count that never shrinks; whole on the pass.
- **Stamp:** a mono uppercase label in a 1px currentColor box (3px radius) for states: Default, Pending, Approved, Rejected, Expired, Owner; the fresh New/Updated stamp takes a Crab Red border.
- **State lamp:** mono uppercase label led by a 7px square lamp in the status color, for status columns on gate boards.

### Cards / Containers
- **Pass card:** Paper, 1px Strong Line border, 14px corners, Pass drop shadow; grid of main column and 10.5rem stub, each drawing its own side of the card, the stub's dashed left border the perforation.
- **Ticket:** Paper card (max 25rem, or full width as `wide`), body above a dashed tear-off foot on Paper Two.
- **Faces:** the pass's view and edit faces share one grid cell, the inactive face hidden but laid out, so the card keeps the taller face's height while switching between viewing and editing.
- **Ink board:** the 7-day departures board under a Link's pass: Shell Ink (the dark band color in dark theme), 14px corners, seven split-flap tiles, then where most Clicks came from as ink rows.

### Inputs / Fields
- **Style:** 2.5rem (2.75rem on the pass), 1px Field Line border, 6px corners, Inter 0.9375rem; mono for Targets, datetimes and read-only values (read-only on Paper Two). Selects draw their own chevron.
- **Focus:** a 2px ring laid over the field's own border (offset -1px), so the box never changes size.
- **Labels:** a micro-label over each field; hints in 0.75rem muted beneath, aligned across a segment row by subgrid.

### Navigation
- **Band:** Shell Ink, 3.75rem, logo lockup at 2.25rem high, tabs Links and Clicks only (Inter 500, muted until current, a 3px inset rule under the current tab), one menu button at the end showing the avatar (and the current Workspace's name once there is more than one).
- **Band menu:** a Paper popover (10px, Pass lift), the account email in mono at its head, then Workspaces (current marked with a dot), the Workspace's Domains, Members & API Keys and New Workspace, Account, Instance Admin for admins, the language as a two-segment toggle whose chosen segment inverts, and Sign out.
- **Phone:** bottom tab bar, see Layout.

### Gate Board (signature)
A table under a 2px ink rule, mono micro-label heads, hairline rows (separate borders, so a rule moves with its row), tabular figures right-aligned. Board rows on Links: the whole row is one pick control; the Domain host muted, the Code bold; Target and meta (Tags, Expiry) beneath on one line. Selected rows invert; fresh rows carry a 6% Crab Red tint and the fresh stamp until picked.

### Boarding Pass (signature)
Route: a muted micro-label over the display-size Code on the left, a flight line (1.5px dashed, at least 5.75rem) with a plane at its end, and the Target's host right-aligned. Segments: a two-column grid of mono values, each row ruled by a hairline above it, the left column with a hairline right edge. A long Target keeps to three lines. The stub: QR scan block (7.75rem, white, 6px), the Short URL in mono that shrinks to fit (never cut), then Copy and Download QR. A blank pass is the New Link editor: Target first, a placeholder Code drawn as empty slots on the flight line, More options for Code, Tags and Expiry, and the stub previews the Short URL and issues with Create.

### Motion
Ease-out `cubic-bezier(0.16, 1, 0.3, 1)` throughout, transform and opacity only, at most 400ms, no loops.
- **Print** (0.34s): a pass enters from 45% opacity and 1.25rem up, never from blank.
- **Fly** (0.4s): the plane crosses its flight line once.
- **Flap** (0.26s per character, staggered so the last starts by 140ms): figures flip in from 40% opacity and scaleY 0.4; only changed characters flip again.
- **Tear** (0.4s): a stub or foot is ripped off along its perforation, on sign-in, on accepting an invite and when a new Link is issued, and never comes back. The rip runs along the line from a random end (0.26s: quick, catching about halfway, then through), leaving both edges ragged and fibrous behind it, drawn fresh each time: the tear wanders a few px off the line, frays, nicks and opens a little. Torn through, the foot comes away, 0.5rem and fading (0.14s, ease-in), and the held half springs back 1.5px. A refused submit (a wrong password, a taken Code) tears it only partway, half the first time and a little further each try after, up to 80%; it stays so, still usable, and the next success rips on from there. Forms that stay on screen (change password, add a Domain, create a Workspace, invite) do not tear. Off under reduced motion: nothing is torn.
- **Land** (300ms after a 120ms delay): the new row slides in from 0.75rem left while the others rerank (360ms FLIP slide).
- **Drop off** (240ms, ease-in `cubic-bezier(0.7, 0, 0.84, 0)`): a deleted row drops 0.75rem and fades before the rest close up.
- **Draw** (0.4s): the dashed loading rule draws out once and holds; **grow** (0.4s) for breakdown bars.
- **Small moves:** menus and dialogs drop in (0.18s), toasts rise (0.3s), controls transition at 0.15s, chevrons rotate at 0.2s.
- **Landed:** right after a create, the pass suppresses its print, plane and flaps; the row's landing owns the moment.
- **Reduced motion:** every CSS animation and transition is off, and the JS row animations do not run.

## Do's and Don'ts

### Do:
- **Do** model a new screen as a gate board (rows under a 2px ink rule) or a pass/ticket on the ground (card, perforation or tear-off foot), never a third container type.
- **Do** set Codes, Short URLs, hosts, counts and dates in JetBrains Mono with tabular figures, and field keys as mono uppercase micro-labels (0.6875rem, 0.08em).
- **Do** keep the pass, tickets and confirm dialogs paper in dark theme, and the band and 7-day board ink in light theme.
- **Do** show selection by inversion and freshness by the Crab Red stamp plus a 6% tint.
- **Do** keep every control 2.5rem high (2.75rem on the pass, on touch screens and on phones) with 6px corners and a Field Line border.
- **Do** keep content that is copied or printed whole: shrink the mono to fit (Short URL to 11px, Code to 1.25rem), then break at a slash or hyphen; never ellipsise it.
- **Do** keep each motion at or under 400ms, ease-out, on transform and opacity, playing once, and off under prefers-reduced-motion.
- **Do** stack alternative states of one card (view and edit) in one grid cell so the card's size never jumps.

### Don't:
- **Don't** use Crab Red for small text, borders of ordinary controls, charts, or more than one element on screen at a time.
- **Don't** build the sidebar-plus-white-cards admin template or a marketing hero; there is no sidebar, and no hero.
- **Don't** put a micro-label above a heading or a paragraph as decoration; it names the field beneath it.
- **Don't** give rows, boards or sections a shadow; only objects on the counter lift.
- **Don't** add texture beyond the world's own: hairlines, dashed perforations and flight paths, the QR block and the flap seam.
- **Don't** loop an animation or let a loader spin; the flight path draws once and holds.
