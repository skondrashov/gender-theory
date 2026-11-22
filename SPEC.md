# gender-theory — the spec

The owner's design, in layers. Decided things are stated; open things are marked **open**.
`AGENTS.md` says how the engine works; this says what it's for.

## Layer 1 — mechanics

The player learns to *see*: each mechanic is an unlock, global across the album, that
changes how every scene is drawn. Unlocks never move you; you watch the one you're on
change, and the door that needs it appears.

**The system** (owner, 2026-11-15): what a cell can be is **dots, lines and rings** — lines
and rings can coexist on one cell (a clock: a ring with a line turning inside it). Each is a
thing that exists, then parameters of it, each its own unlock. Lots of minor unlocks is the
point: a player needn't notice they got one — it reads as the game progressing. A static
look is free from the start (a line points wherever its layer says, dots appear or not
without glow); an unlock lets a parameter leave its default or move.

**One system** (owner, 2026-11-18: "i want everything really structured so there's a
relatively simple uniform system for the entire thing — one unlock unlocks one row"): a cell
is thirteen **rows**, one per unlock — brightness, dots; lines, length, angle, bend; rings,
radius, fill, morph, flatness, rotation; color. An unlock is a row, or a row's sidechain
option — glow is brightness's, dots is dots's (2026-11-20: "dots should just be treated like
other unlocks for simplicity. that way i can disable dots on some cells"), angle is angle's
(owner: "an unlock basically just means either a
row or a sidechain option, so glow is really just the brightness sidechain option").
Refined 2026-11-22 — "conceptually, an unlock is either the ability to set a value, or the
ability to automate it … the purist in me still wants to stick with 1, and force myself to
work within the more restricted system": every unlock is the ability to **set** a row (it
rests at 0 until then) or to **move** it (its music sidechains, masks and spin wait), in pairs
where a row has both — dots/glow for brightness (dots first: everything is dark before it),
lines/length, angle/rotation, rings/size — and single for bend, fill, morph, flatness, color;
the dots row and the ring's rotation row are free. A layer may also `need` unlocks outright,
for a pattern that changes with one. A
layer is a region plus the rows it changes, each
*set*, *added to* or *multiplied* over what the layers before it left; a cell under no layer
is a dark dot (dots always exist; brightness can be 0). Any row can be masked by distance,
bearing or the way round the layer's rectangle (fixed patterns, always on), by a hotspot
showing (a hint) or an unlock being had, and moved by the music (comes in with its
unlock). **Position** is a source in its own right (owner, 2026-11-18: "rather than
distance and angle, it should just be position with distance and angle as parameters and
then a ton of polar coordinate functions with parameters … full sidechain options on each
one"): a library of fields — ripple, spiral, beam, orbit, rain, … — each a function of the
cell's place with a centre, a facing, a phase and its own parameters, every one of them
sidechainable, so patterns breathe, spin and travel with the music. A new layer is one row: brightness 1. Nothing else decides whether a layer shows
(owner: "that will always be controlled by brightness") — only its stage and bars scope it.

- global: **glow** (the owner's "opacity": brightness gets to follow the music; nothing
  breathes unless a layer says so),
  **color** (one color, a range from white to it; more colors = more unlocks), and later
  **root position motion** (the mark's origin leaves the lattice — "probably actually way
  later"; not built)
- **lines**: points become lines, pointing where the layer says, one cell long →
  **length**, **angle** (they turn), **bend** (into arcs)
- **rings**: a circle, 0.4 cells → **size**, **fill** (outline → disc), **morph**
  (circle → square), **flatness**, **rotation** (they turn)
- **hexes** (the square grid sheared into a triangular lattice — the next song's mechanic;
  not built)

The first act teaches glow → lines → length; after that the order is free, per song
("the order shouldn't really matter"). **open:** which song teaches which. Each reveal
eases in over 2.5 s: the line grows out of the point, stretches, winds up to speed.

A mechanic is introduced by a phase built to teach it: the hint is in the drawing (the
blocky e, the glowing gap in the stripes), the action is a tap or a stroke, and the phase
visibly changes under you when you get it.

## Layer 2 — songs

Each song has a **theme**: a visual/mechanical idea particular to it that its phases
develop. A song is cut into phases (`tools/phases.py` finds them; the owner corrects);
each phase loops a bar range and has its doors.

Going back to a song with a new mechanic should let you do new things there: phases carry
hotspots that `need` mechanics the player didn't have the first time, invisible and inert
until then.

| song | theme | mechanics it teaches | state |
|------|-------|----------------------|-------|
| sine | the first act: words → square → stripes → boxes → cross | glow, lines, length | built (5 phases, in album.json), 6th placeholder |
| sasha grips | satellites: two points orbit every dot; the bass pulls the line between them | — | the growly part built; rest placeholder |
| pisser | **open** (opens with 8 bars of near silence, then builds without bass) | **open** | placeholder |
| for.against | **open** (the project's 'drop' at bar 77; a filtered break at 37) | **open** | placeholder |

## Layer 3 — the album graph

Songs are an illusion; the player browses the album. Paths:

- **linear**: through a song in stretches. A stretch of the track lasts as long as it takes
  to get through a few stages (visual states, tapped through, the music going on), then
  loops its tail waiting for the trigger into the next part (owner, 2026-11-05);
- **out the middle**: some paths to a new song leave from the middle of another, not its
  end — **open:** which;
- **back**: returning to a song later, with more mechanics, is meant to be worth it (Layer 2);
- **loops**: the album's end leads to its start.

Transitions — the music of leaving one phase for another — are **deferred**: real
transitions can be many bars long and the player shouldn't wait; speeding them up by
interpolation probably won't work on most tracks. Changes land on the next bar, crossfaded.

## How the album is written

The album is data (`static/album.json`), made in the editor (`/editor.html`): per track, the
scenes (loops), the layers (a region, a mechanic, its parameters, its bars) and the hotspots,
all in bars. Any parameter can be **sidechained**: it takes a base value plus any number of
sources — frequency bands the owner defines in one place (Hz range, attack, release, gain),
each giving its level and the **pitch** of what's loudest in it (as a position in the range,
and as a note), or the built-in events (beat, bar, pulse) — each through threshold, width,
curve and depth.
So "length follows the bass" or "gray breathes with the kick" is a knob, not code; code is
for new *kinds* of mechanic (`scenes/data.rs`).

## Rules of the drawing

- Music is events, not waves: each bar, beat and subdivision hits and falls away. Nothing
  pulses on a sine; everything that pulses goes through `pulse()` (beat hits, downbeat
  hardest, smoothing allowed) or follows a band of the music itself.
- A line is a cell's mark; what it can be is what's unlocked. Dots and stripes that must
  stay what they are regardless are scenery (`dot`, `stroke`).

## Rules the engine enforces (so the spec can rely on them)

- A hotspot shows only in its stage (if it has one) and when the player has all it
  `needs`, and disappears once it has nothing left to do (an unlock already had); a second
  hotspot on the same cells can be the door that needs it.
- A stage change lands at the next bar, morphing like a scene change; the music is
  untouched. A scene change restarts the new stretch from its first bar.
- From a trigger until the change lands: no input, the scene morphs into
  the next over the last 1.5 s.
- Progress (scene + unlocks) is saved in the browser. **M** opens the whole graph. The
  editor's graph tab is the same graph, editable (doors drawn between scenes).
- `scenes::dangling()` reports doors to nowhere at startup. **open:** a check that every
  `needs` is reachable before the phase that needs it (the spine is honoured).

## The grid (owner, 2026-11-18: "i love 11x11 also, so let's lock that into the editor along with 3px")

- **11 × 11** cells: odd, so there is a centre dot to build on (rings round it, centred on
  the screen). The grid scales to the screen: pitch = shorter side ÷ 11 (≈ 36 px on a phone,
  98 px at 1080p). **open:** what happens in the leftover strip of a non-square screen
  (black for now).
- Dots **3 px**; lines **3 px, round caps**, so a line covers its dot exactly and no more.
- **Distance and angle**: anything can be a function of a cell's distance from the centre
  dot or its bearing (sources `distance`, `bearing`), and the beat can arrive later further
  out (a chain's `ripple`) — ripples. Layers stack, each on its own cells, setting, adding to
  or multiplying any row; so a missing dot is a dot nothing lights, and a ripple only
  touches the cells its layer is painted on (owner: "these masks can stack and each mask is
  enabled on certain cells with the same editor as the hotspot and layers").
- The old 20 × 20 designs were rescaled to 11 × 11 mechanically; act 1 needs redrawing.

## Open questions, collected

- The hidden box in sine's 4th phase: what does tapping it do?
- After `length`: the order of rotation / circles / color / hexes, and which song teaches which.
- Where the mid-song exits are.
- pisser's and for.against's themes.
- Transitions (deferred by decision).
