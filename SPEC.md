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
| sine | the first act: words → square → stripes → boxes → cross | glow, lines, length | built (5 phases), 6th placeholder |
| sasha grips | satellites: two points orbit every dot; the bass pulls the line between them | — | the growly part built; rest placeholder |
| pisser | **open** (opens with 8 bars of near silence, then builds without bass) | **open** | placeholder |
| for.against | **open** (the project's 'drop' at bar 77; a filtered break at 37) | **open** | placeholder |

## Layer 3 — the album graph

Songs are an illusion; the player browses the album. Paths:

- **linear**: phase to phase through a song, tap the door;
- **out the middle**: some paths to a new song leave from the middle of another, not its
  end — **open:** which;
- **back**: returning to a song later, with more mechanics, is meant to be worth it (Layer 2);
- **loops**: the album's end leads to its start.

Transitions — the music of leaving one phase for another — are **deferred**: real
transitions can be many bars long and the player shouldn't wait; speeding them up by
interpolation probably won't work on most tracks. Changes land on the next bar, crossfaded.

## Rules the engine enforces (so the spec can rely on them)

- A hotspot shows only when the player has all it `needs`, and disappears once it has
  nothing left to do (an unlock already had); a second hotspot on the same cells can be
  the door that needs it.
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
