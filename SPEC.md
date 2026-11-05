# gender-theory — the spec

The owner's design, in layers. Decided things are stated; open things are marked **open**.
`AGENTS.md` says how the engine works; this says what it's for.

## Layer 1 — mechanics

The player learns to *see*: each mechanic is an unlock, global across the album, that
changes how every scene is drawn. Unlocks never move you; you watch the one you're on
change, and the door that needs it appears.

The spine, in order, regardless of music:

1. **glow** — brightness breathes with the music; some areas breathe in inverse
2. **lines** — points become lines (still, along the grid, one cell long)
3. **length** — line length moves

After the basics the order is free; they can come in different orders, in different
songs. Known so far: **rotation** (lines move — a still line may point anywhere, and
"rotation" means motion), **circles**, **colour** (one colour, a range from white to it;
more colours = more unlocks), **hexes** (the square grid sheared into a triangular lattice
— the next song's mechanic). **open:** which song teaches which, after `length`.

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
sources — frequency bands the owner defines in one place (Hz range, attack, release, gain)
or the built-in events (beat, bar, pulse) — each through threshold, width, curve and depth.
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
- From a trigger until the change lands: white border, no input, the scene morphs into
  the next over the last 1.5 s.
- Progress (scene + unlocks) is saved in the browser. **M** opens the whole graph.
- `scenes::dangling()` reports doors to nowhere at startup. **open:** a check that every
  `needs` is reachable before the phase that needs it (the spine is honoured).

## Open questions, collected

- The hidden box in sine's 4th phase: what does tapping it do?
- After `length`: the order of rotation / circles / colour / hexes, and which song teaches which.
- Where the mid-song exits are.
- pisser's and for.against's themes.
- Transitions (deferred by decision).
