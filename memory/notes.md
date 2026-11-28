# gender-theory — notes across sessions

## Build

- Toolchain that works (2026-11): rustc 1.93, wasm-pack 0.11.0, nannou git rev `287be8a` (2022,
  `wasm-experimental` → wgpu 0.11 on WebGL2), wasm-bindgen **0.2.92** in the lockfile (0.2.83
  is refused by current rustc; 0.2.88 is yanked). `wasm-opt = false` in `app/Cargo.toml`
  because wasm-pack 0.11's bundled binaryen can't validate wasm from current rustc.
- `cd app && wasm-pack build --target web` → ~25 s incremental, written straight into
  `app/pkg`, which is what's committed and deployed: check it in a browser before committing,
  `git checkout app/pkg` if it's bad. (Until 2026-11-04 `app/.gitignore` ignored `/pkg`, so
  despite what the old AGENTS.md said it was never in git — only on this machine. It's tracked
  now, ~2.5 MB of wasm per rebuild.) wasm-pack rewrites `app/pkg/.gitignore` to `*` every
  build; that file is deliberately untracked (tracked files ignore ignore rules).
- Headless check without a browser session: Edge works —
  `msedge --headless=new --use-angle=swiftshader --enable-unsafe-swiftshader --screenshot=... --virtual-time-budget=8000 URL`
  (segfaults on exit, after writing the screenshot; harmless). No audio, no clicks, so only
  rendering and startup console errors can be verified that way.
- Test server for a scratch build: any static server with Range support (Chrome's media
  element wants it). Python's `http.server` doesn't do Range; a 30-line handler does. But
  the editor's save needs the real dev server (`cargo run`, PUT /album.json), so use that.
- The root crate's lockfile had `time 0.3.21`, which current rustc refuses (E0282); `cargo
  update -p time` fixed it (2026-11-05). `cargo run --release` builds in ~1 min.
- The album is `static/album.json`; the wasm exports `editor/state/spectrum/catalogue` for the
  editor page. Driving the game from a script: open `/?editor=800` (fixed 800 px canvas,
  nothing saved), click (400, 400) to start, then `import('./app/gender.js').then(m => m.state())`
  tells you the scene, bar, bands, visible hotspot tags. Cell (i, j) is at
  (400 + (i − 10) × 40, 400 − (j − 10) × 40).
- Claude Code's Bash tool chokes on heredocs whose body contains backticks (bash reports an
  unterminated quote): write patch scripts with the Write tool and run them with python.

## Tracks

The real music lives in `Desktop\gender theory\` (mp3 bounces + Ableton projects; the
project folder `gender theory\gender theory\` has the paired-word projects: being.useless,
for.against, porcelain.piss, terra.firmament, violence.release). Two bounce names contain
U+F022 where a dot should be (`for<U+F022>against.mp3`, `terra<U+F022>firmament.mp3`).
`static/music/` holds the copies the app uses — the owner's audio frames untouched, with one
**Info frame prepended by `tools/gapless.py`** (see below):

| file | source | bpm | bars | grid |
|------|--------|-----|------|------|
| `music/sine.mp3` | `gender theory/sine.mp3` (2023-02) | 135 | 84.5 | bar 0 at t=0 |
| `music/sasha-grips.mp3` | `gender theory/sasha grips.mp3` (2023-03) | 147 | 88.25 | bar 0 at t=0; no Ableton project found |
| `music/pisser.mp3` | `gender theory/pisser.mp3` (2023-01) | 120 | 104.0 | bar 0 at t=0; the porcelain.piss project is later and at 130 — different version |
| `music/for-against.mp3` | `Desktop/for.against (for mix).mp3` (2023-05-25, newest bounce; the flac of it is 2023-06-05) | 128 | 150.5 | bar 0 at t=0 = project bar 13; project 'drop' locator = bar 77 here |

Tempos: sine/pisser/sasha grips from the audio (`tools/phases.py`; beat-grid fit decisive),
for.against from the project (every version says 128; the audio fit is mushy — dense and
reverby). Other bounces of for.against: `Desktop/for.against.mp3` (05-08), `instrumental
final` (05-15), `bounces/for.against.mp3` (05-09, 5:14, longer), `gender theory/for<U+F022>against.mp3`
(03-16, 145 bars — an earlier cut). Also around: `Desktop/piss.mp3` (2024-08, 4:20, probably
the porcelain.piss version), `terra<U+F022>firmament.mp3` (146 bpm project), `4teat*`, `bungus*`.

Phase maps (per-4-bar energy from `tools/phases.py`; the cuts in `scenes/album.rs`):

- **sine**: 1–16 no bass · 17–64 full in 16-bar blocks (layers added at 17/33/49 in the
  project) · 65–72 bass out, highs out · 73–84 just the tone, fading.
- **sasha grips**: 1–8 brightest · 9–32 · 33–60 mids up from 37 · 61–64 bass drop · 65–88.
- **pisser**: 1–8 near silence (sound from 4.7 s) · 9–24 build, no bass, highs rising ·
  25–88 four 16-bar cycles, each dipping in its last 4 bars (37, 53, 69, 85) · 89–104 outro.
- **for.against**: 1–12 no bass · 13–36 groove · 37–44 highs filtered out · 45–60 · 61–76
  build (mids up) · 77–108 drop · 109–116 breakdown, bass out · 117–148 way out · 149–151 tail.

Old excerpts (`bu/2`, `fa/1`, `fa/2`) keep their hand-counted grids as bpm (73.2802,
62.5, 109.375 — i.e. 8, 14, 14 "bars" over the file). `tf/` files are byte copies of `fa/`.

**Gapless header (2026-11-05).** Ableton's mp3 export writes no Xing/LAME Info frame, so
decoders can't trim the encoder delay: measured against `Desktop/for.against.wav` (same
bounce as the 05-08 mp3), the decoded mp3 starts **2258 samples = 51.2 ms** late — 1729 of
encoder delay + the 529 every decoder adds. Chromium's `decodeAudioData` showed the same
2258 on all four tracks. So bar 0 was 51 ms into the buffer and every loop point and switch
landed 51 ms early: the seams. `tools/gapless.py` prepends a standard Info frame with a LAME
tag carrying delay 1729; Chromium then puts sine's first sample at 0 (verified over the
DevTools protocol; Firefox/Safari honour the same tag). The old excerpts (`bu/ fa/ tf/`) were
left alone — their hand-set grids are relative to the whole file. If the owner re-exports a
track, run `gapless.py` on the new file (idempotent: it skips files that already have one).

Tooling: `tools/phases.py` (numpy + miniaudio, `pip install --user numpy miniaudio`, run
with `C:/Python312/python.exe`; no ffmpeg on this machine). `.als` files are gzipped XML:
tempo at `MasterTrack//Tempo/Manual`, `Locator` elements, clip `CurrentStart/End` in beats.

## Decisions (owner, 2026-11-04)

- Reviving, not archiving. Technical side handed to the agent; owner designs scenes.
- Unlocks are global across the album; songs are an illusion, you browse the album.
- Taps land on the next bar. Input block from trigger to landing (the white border was
  dropped 2026-11-18: "the transitions look smooth enough now and telegraph what's going on").
- Unlocks never move you; the door that needs them appears on the same scene.
- A transition intensifies the *current* scene with a passage of the *current* track.
- Rotation means movement; a still line may point anywhere.
- `COLOR` in `scenes/mod.rs` is a placeholder orange — owner to choose.
- Old scenes kept as `old/1`–`old/6`.
- Scene changes morph rather than snap: the last `MORPH` = 1.5 s before landing (less if
  asked for later — a late tap gets a quick morph, never none), ending exactly on the switch.
  Menu jumps land 1.5 s out so they morph too. The old scenes draw on `draw` directly, so
  they snap; new scenes must go through `mark()`/`ring()` to take part.
- Real music first, phases → scenes with loops; **transitions later** (the owner: the real
  transitions can be very long, speeding them up by interpolation probably won't work on
  most tracks; don't make the player wait). Owner can supply approximate or exact times.
- **The first act** (owner's scene list, 2026-11-04; built on sine's five phases):
  1. "met ends" in point-letters, all still (first frame: nothing but points). The e in
     "met" is blocky — strokes a unit thick, two points wide — the rest one point thick.
     Tap that e → on.
  2. A square; tap → **Glow**: the whole field breathes on the half-bar (0.35..1), a second
     square breathes in inverse (black at the peak). Tap it → on.
  3. An inverse-glow patch; tap → **Lines**: the field becomes stripes (every other row)
     with a gap of bare points; draw through them (one-row `Area` + `Outline`) → on.
  4. Checkerboard of unit lines (along the grid, still) and dots. The one closed boundary
     the lines make is a box: tap it → on. A large region glows inverse; hidden in it a box
     whose cells stay in the greys. **Open: what the hidden box does** (inert for now).
  5. Same field with a cross; tap the centre → **Length** (lengths move with the music);
     tap again → on. "Stop here for now."
  Lines so far: stationary, horizontal/vertical, one cell long — scenes choose angles.
- Seams (2026-11-05): after the gapless header the owner still heard a click and no fade —
  there was none: one looping source, hard `stop()`. `audio.rs` now plays one source per
  pass with gain ramps (20 ms in before the bar line, 30 ms tail at a wrap, 80 ms tail on a
  cut). Constants to taste; a longer `CUT_TAIL` means hearing the old track's next downbeat
  under the new one. Headless Chromium (DevTools protocol, `cdp_click.py` in the session
  scratch) ran two wraps and a switch without errors — but only ears can judge the seams.
- sasha grips' theme (owner, 2026-11-05): **satellites** — two points orbit every dot at
  their own rates (one near, one far, opposite ways); a line runs dot → near point and is
  pulled toward the far one as the bass comes up, back as it fades. Built for the growly
  part (`sasha-grips/2`, bars 33–60); the pull maps bass 0.25..0.75 → 0..1, smoothstepped.
- Headless Edge for tests: launch with `--mute-audio` (a straggler once played sine out of
  the owner's speakers) and kill it by the `msedge.exe` PID found via its `--user-data-dir`
  (`Get-CimInstance Win32_Process`), not by the bash job — `kill $EDGE` only takes the
  wrapper. Never kill all msedge.
- Headless screenshots are unreliable for scenes on the bigger tracks: the virtual-time
  budget races the real decode and the DevTools `captureScreenshot` starves under software
  GL. The app logs `loaded <path>` on every decode; read that over DevTools (`cdp_click.py`)
  to confirm loading, and trust the owner's browser for the picture.
- Next song (owner): **hexes** as an unlock — the square grid sheared into a triangular
  lattice (rows slid half a cell, rows squashed by √3/2); regular hexes can't align with a
  square grid, the shear can. Not started.
- Glow is `Unlock::Glow` (bit 5, appended after Color so saved progress keeps meaning);
  `glow(model, pulse, inverse)` is the multiplier, `dot()`/`stroke()` draw scenery that
  ignores unlocks (dots stay dots next to lines).
- **An editor, and sidechains** (owner, 2026-11-05): "select regions, mechanics, enter
  parameters, and associate each thing with a section of the track measured in measures";
  then "any number of frequency ranges … a list of frequency ranges in one place, and in
  another a list of parameters, and you can use like a sidechain toggle to get the input for
  that parameter from one of the ranges … a depth knob and other sensible knobs". Built as
  `static/album.json` + `scenes/data.rs` + `static/editor.*`; the first act and the
  satellites were migrated from Rust into the file (the Rust versions are in git history,
  `scenes/album.rs` before 2026-11-05). Knobs per sidechain: depth, threshold, width, curve,
  invert, on; per band: attack, release, gain. The layout is a first draft the owner asked
  to see; expect to rework it.
- **The snake, and presets** (owner, 2026-11-06): "all the points are connected in
  horizontal lines. the lines retreat in such a way that it looks like it's all one singular
  line retreating and leaving dots, maybe even in alternating directions along the y axis
  … then, each dot rotates and extends a line to re-connect in a line along the x axis, but
  does it in the inverse way so it looks like singular lines are coming in" — built as the
  `snake` kind (one polyline through the region's rect in serpentine order; cell k owns the
  segment to cell k+1; `progress` follows the new `span` source = 0..1 across the layer's
  own bars). Then: "maybe what i want is like saved scenes and layers in like a preset form.
  layers i think is the right level" — so it's not placed in a scene; it's two **presets**
  (`album.presets`, editor-only): `snake · retreat` and `snake · arrive` (spin 180,
  reverse). The retreat anchors at the next dot and pulls back; the arrive grows out of each
  dot, turning `spin` degrees as it goes.
- **Painting is the default** (owner, 2026-11-06): "if i'm editing a hotspot or layer, i
  should be able to just click on the map. and default it to empty and 'map' setting instead
  of 'whole'" — new layers and hotspots start as an empty map and are painted on the grid;
  hotspots have a `region` like layers (`Hotspot.inside` narrows the bounding `cells`;
  outline/hints use the rectangle round it; old `rect` still reads). The drag-paint bug was
  an operator-precedence inversion in the grid's mousemove.
- **11 × 11 locked in** (owner, 2026-11-18): engine `CANVAS_WIDTH/HEIGHT` = 11,
  `Model::scaling` f32 = min(window w, h) ÷ 11 every frame (editor: px ÷ 11), `DOT` 3 px,
  `WEIGHT` 3 px round caps, a zero-length line draws as the dot; cell (5, 5) is the window
  centre. Album regions rescaled 20 → 11 by a script (rects × 11/20, maps nearest-sampled):
  act 1 is garbage at this size and needs redrawing by the owner. Also: sources `distance`
  and `bearing`, chain `delay` (phase sources shifted by delay × distance), glow layers
  stack with `op` multiply|add, rings `count`/`gap` (halos), marks angle `radial`/`tangent`.
  The owner's model: "a basic layer is just a gray value of 1 that isn't sidechained in any
  way, and then any layers on top can apply transformations to that layer" — so new marks
  default to gray 1 and the masks are the glow layers.
- **Grid and dot size** (owner, 2026-11-18, from the Grid Sizes page,
  https://claude.ai/artifact/MQwKA832rW6kBjGQXUGWDQ): "3px definitely seems correct. the
  lines will have to be the appropriate thickness to completely cover the dots but not more"
  → dots 3 px, lines 3 px with round caps; "for grid size i think 11x11 (i want an odd number
  so i can have a central dot … rings around it that do cool things centered on the screen)".
  Grid scales to the screen: pitch = shorter side ÷ 11. The effects sampler at that size is
  https://claude.ai/artifact/F4yCxLNdbJHU6XLxQHAcnV (canvas mock of every engine effect plus
  centre-dot ideas). Engine still 20 × 20 at 40 px, 1 px lines — to move once confirmed.
- **Rows** (owner, 2026-11-18: "why does it feel like we're still treating glow like a
  separate value from stuff like line angle? … one unlock unlocks one row, that row can be
  masked using a distance and orientation function for each parameter, we start with only one
  and it's called … 'brightness' … make it so that dots always exist, but they can have a
  brightness of 0"): the kinds marks / points / rings / glow are gone; one kind, `cells`, with
  twelve rows (`ROWS` in data.rs, one per unlock, `catalogue()` feeds them to the editor).
  A row = `{op: set|add|multiply, base, chains, spin?, alternate?}`; a layer carries only the
  rows it changes; the cell is folded over the layers in order (a layer's gate factor scales
  its change), then each row is gated by its unlock: brightness and angle `Before::Static`
  (chains on the music and spin × reveal), the rest `Before::At(v)` (eased from v). Line =
  length × lines; ring = radius × rings; lines 0 is the dot. Source `around` replaces the
  `around` angle name; `radial`/`tangent`/`noise`/`checker` angles are chains on bearing /
  noise (checker became noise > 0.5 — a different random half). Snake, orbits and walk stay
  as they were (gray → brightness) and rows don't reach them. Migration: old glow layers →
  brightness × / + rows, moved after the other layers of their stage (they used to act
  regardless of order); rows at their start value were pruned unless another layer of the
  scene sets that row. Editor: "showing when" first, rows with an op/base/spin/unlock tag,
  "+ row" picker, grid "clear" button. Dropped: `ignore_glow`, rings `count`/`gap` (halos),
  `gated` scenery rings.
- **No "showing when"** (owner, 2026-11-18: "that will always be controlled by brightness"):
  a layer's `when` / `fade` / `needs` are gone. A row has `masks` (sidechain-shaped; the
  product of `1 − depth × (1 − shape)` scales how much of the row's change lands —
  `Row::amount`) and two state sources: `hotspot:<id>` (1 while that hotspot of the scene
  shows) and `unlock:<name>` (its reveal). Both are "fixed" like noise/distance (act before
  glow). A hint = a brightness row masked by its hotspot. Migrated: each gate → a mask on
  every row of the layer; hint layers lost rows equal to the field's. The owner's worry
  ("we can't easily contingent the display of the row on what unlocks it uses") is answered
  by the unlock mask. Layer scope is now only `stage`, `bars`, `muted`.
- **An unlock is a row or a sidechain option** (owner, 2026-11-18): `Before` → `Gate
  { Sidechains, Row(v) }`; glow and angle are the sidechain options of brightness and angle
  (free from the start; the music, masks on the music, and spin wait for the unlock), the
  other ten unlocks are rows (rest at `v` until then). Editor shows the unlock tag on the
  row or on its sidechain button accordingly. Play now drops a layer/hotspot selection for
  the scene, so the region squares leave the preview.
- **Fields** (owner, 2026-11-18: "i wanna be able to like sidechain the depth of the
  distance and shit … rather than distance and angle, it should just be position with
  distance and angle as parameters and then a ton of polar coordinate functions with
  parameters … get creative … err on the side of variety over perfection, with some clear
  motivation for each effect … full sidechain options on each one"): `scenes/fields.rs` —
  `FIELDS` (41 specs in families radial / angular / spiral / linear / pairs / bodies /
  scatter), `Field { fn, params: HashMap<String, Param> }`, `Field::eval(signals, gate,
  depth)`; a `Sidechain` with `source: "position"` carries `field`. Common params x, y,
  turn, phase, shape, edge, sharp; `Signals` now carries i, j, di, dj, time; new ramp
  sources `bar.phase`, `beat.phase`, `clock`. Fields nest up to 3 deep (a field param's
  chain can be a field). Contact sheet of all 41 at 11 × 11 was checked headless
  (scratchpad `cdp_fields.py`). Editor: source select has optgroups per family; a field's
  params unfold under the chain with their own sidechain buttons.
- **The 'left' stage of sine/1: turning bars** (owner, 2026-11-18; built by scratchpad
  `left_stage.py`, group `turning bars`): two 8-bar cycles in the 16-bar loop — A on the even
  columns growing up, B on the odd ones growing down; each: arrive 2 bars → fold 2 → collapse
  2 → rest 2. The fold is cells layers (lines/brightness masked by `unlock:angle`, angle 90 or
  270 with a span chain ±90: neighbouring columns pivot toward each other into short
  horizontal bars — a true per-dot pivot can't make full-width bars, the owner chose pairs
  turning opposite ways); without angle the vertical bars hold and collapse as they are
  (snakes whose brightness is `0.9 − 0.9 × unlock:angle`). The folded bars' collapse is a
  snake per block with `heads` = rows (bars in neighbouring rows touch, so a block is one
  piece; heads cuts it into a run per row), with `tail` so a bar c0–c1 is the cells c0..c1−1. One segment (dot 4, row 5, cycle A) is missing
  from the fold: a drag hotspot (needs angle, bars 2–6) from it to the next dot unlocks
  `length` (placeholder choice), and a layer shows the drawn segment once length is had.
  Snake `dots` 1.0 to match the stage's base dots (a dot of brightness 0 would paint black
  over them: marks draw in order).
- **The `bars` kind** (owner, 2026-11-18: "maybe i'll just let you write some of these
  algorithms and use them as kinds of layers. so make a kind of layer where alternating
  columns grow all the way off the grid from dots on alternating sides of the grid, then all
  the lines rotate to make alternating lines of horizontal bars which retract, rotate, then
  grow off the grid again, etc"): `Kind::Bars` in data.rs. The figure (bars along every
  `step`th column, the first from the bottom, the next from the top …) is drawn in its own
  frame; first version turned the whole figure rigidly about the centre — owner: "bro no…
  it's not that the whole figure turns, it's that each individual line rotates … you're not
  thinking trippy algorithmic art enough"; second version turned each whole bar about a
  point of the diagonal; the owner then spelled it out: "every other dot on the bottom grows
  a line upward, and when it touches the next dot, that dot grows to the next dot etc. on
  the columns where this isn't happening, a line extends down from the top dot … all of the
  line segments that make up the alternating lines rotate in such a way that they form
  alternating lines horizontally, and then the lines retreat in the reverse animation of the
  growing, the dots 'rotate' which you can't see, and then extend lines vertically again".
  So: per cell, one segment per dot (the last off the grid, else the top and bottom rows
  would come out dashed); grow: fill = clamp(g·11 − n) with n the dot's place from its
  column's start; turn: each segment pivots ±90° on its dot — even rows end pointing right,
  odd rows left, which is what tiles 11 dots into a full row either way; retreat: the same
  fill with g falling and n counted from the row's start; fourth phase nothing. Then
  (owner: "extend the cycle so that the lines alternate the other way when they're vertical
  again? and then horizontal again? … each beat does one thing … extend rotate retract
  pause? since it's 4 count") the second half with both parities flipped — eight phases,
  on `Signals::bars × 4`; a cell-a-beat schedule was tried ("weird because i have 11
  dots. forget that then, let's just do 4 beats per part"), then the pause dropped ("i'm ok
  with it being 3 parts actually, remove the pause") — so: six parts of `per` beats
  (default 4 = a bar: extend, rotate, retreat, then flipped; a half-cycle 3 bars, the cycle
  6 — which doesn't divide a 16-bar loop, so the figure drifts against the stretch's loop
  until the music wraps; the owner knows), `progress` × 6 when `per` is 0. Params: per, progress, brightness, dots (0.6), color. Replaced the 15-layer `turning bars` group (and the
  missing-segment hotspot) in sine/1's left stage with one `bars` layer; the fill-in-a-gap
  unlock idea is open (a `gap` param would be the way).
- **Snake `tail`** (owner, 2026-11-18: "i want the effect to extend the last dot in the
  sequence too for this, maybe call it bars instead of snake idk"): bool param; the last cell
  of a run owns a segment too, carrying on the way the line was going (along the axis for a
  lone cell), past the grid's edge if it must — so a bar is as long as its dots and a run
  finishes its arrive/retreat at progress 1 instead of (n−1)/n. Default off (old snakes
  unchanged); the left stage's snakes have it on. The kind keeps its name.
- **A snake per piece** (owner, 2026-11-18: "if i set a layer to be every 2 cols from 0
  with a retreat or arrive effect, i'd want each region to get its own effect … or really
  for any split map"): `SnakePaths` on `LayerRt`, built at install — the region's cells
  flood-filled into 4-connected pieces, each in serpentine order within its own rectangle
  (so a plain rectangle is exactly the old order), `heads`/`progress` per piece. A checker
  region is all singletons: dots, no line.
- **Layer groups** (owner, 2026-11-18: "would be nice to be able to group layers into
  groups, since i use a bunch of layers to make certain sections"): `layer.group` (a name;
  the engine ignores it). Editor: header li over each contiguous run in the list (fold/unfold
  per scene in memory), `sel = {type: 'group', index: name}`, `groupOf(name)` = every layer
  of the scene with that name; the inspector renames/stages/mutes/moves/duplicates/deletes
  them together and adds a layer after the group's last; the preview highlights the union of
  their cells. Draw order is untouched: a group is a label, not a container.
- **The graph tab** (owner, 2026-11-18: "i feel like at some point i'm gonna want a graph
  i can edit of how all the scenes are connected" → "do it"): a tab beside the timeline in
  the bottom panel (`gt-view` in localStorage), SVG, `renderGraph()` in editor.js. Nodes =
  scenes (all tracks), `s.graph = {x, y}` saved only once dragged (serde ignores unknown
  fields, so no engine change); edges = `go`/`via` actions; reachability BFS from the first
  scene of the first track (how `Model::new` picks the start); ghosts for targets not in
  the album, red unless `catalogue().elsewhere` (the hand-written scenes) has them. Drags:
  node move, port → door (`{name: 'door', on: 'tap', region: empty map, then: [{go}]}`),
  arrowhead → retarget. Checked headless with scratchpad `cdp_graph.py` (door made,
  retarget, move saved, edge click selects the hotspot). The game's M menu is still a list;
  using the saved positions there is an offer, not done.
- **+ masks** (owner, 2026-11-18: "i wanna be able to add pulse instead of multiplying.
  cause i want glow effects to work on stars that are blacked out by default"; chose
  "masks can add" over a recipe): `Sidechain.add` (file `"add": true`), editor × / + button
  on each mask. `Row::amount` = clamp(Π × masks + Σ + masks' shares), where with only +
  masks the product term is 0 (nothing lands until a + says so). The readout shows `+ 0.35`
  for a + mask. The owner's 'top' layer in sine/1 foyer (brightness add 0, pulse mask depth
  −5.5) was the symptom — a mask can't light anything on a change of 0; with + it needs a
  base too (set/add 1, + pulse).
- **No transition border** (owner, 2026-11-18): the white 8 px frame from trigger to
  landing is gone; the morph is the telegraph. Input is still blocked meanwhile.
- **Seeded noise** (owner, 2026-11-18: "it'd be cool to set a seed for the noise, also in
  some kind of sidechained way … flashing new random patterns each pulse looks cool"): field
  `noise` — `hash3(cell, seed)`, `seed` a param (a new pattern per whole number crossed: chain
  it to the kick with a hard threshold and it flashes), `every` beats for a fresh pattern
  locked to `Signals::bars` (gated like motion), `grain` for blocks. The plain `noise` source
  (the noisemap) stays.
- **Probe readouts** (owner, 2026-11-18: "i'd like the rows to show me a numerical value of
  how much they're adding to the effect with their current settings"): `Model::probe` (a
  cell, default the centre; editor command `{probe: [i, j]}`, moved by hovering the region
  grid) and `Model::probed` (what each cells row of each layer was worth there this frame:
  value, op, mask amount), reported as `probe` in `state()`; the editor shows `= 0.73 · 85%`
  per row (`.readout`), `—` when the layer isn't on that cell. The probe rests on the selected
  layer's/hotspot's nearest-to-centre cell (`defaultProbe`), drawn orange on the grid; hovering
  the grid moves it, leaving puts it back (owner: "i don't see the readout on the rows for the
  layer 'top'" — the centre wasn't in its region). Each sidechain and mask row has a readout
  too (`Param::parts`, `Row::mask_parts` → `chains`/`masks` in the probe), since the JS
  meters can't know noise, distance or a position field.
- **One stage setting** (owner, 2026-11-18: "i rarely if ever care about the 'all stages'
  stage view … i'd just like it to show whatever stage is currently being previewed"): the
  lists' stage filter is gone; `stageFilter` = `viewedStage()` (the top bar's picker when
  the edited scene is the one playing, else the scene's first stage). New layers/hotspots
  go into it.
- **Gray vs glow** (owner, 2026-11-18: "the controls for setting glow are kinda weird cause
  i don't get the difference between glow and gray in that context"): a draw layer no longer
  has an "own glow" param. One brightness, `gray`; its sidechains (except `noise`) are scaled
  by the glow unlock's reveal, so a layer breathes by sidechaining gray and only once glow is
  had. Glow layers stay (a region-wide dimmer, gated); a draw layer can `ignore_glow`.
  Migrated act-1's own glows: the 'off' ones → `ignore_glow`; the inverse ones → gray chain
  pulse × −gray; `met`'s `1 + span` glow was clamped to nothing and was dropped.
- **Autosave** (owner, 2026-11-15: "can we implement like an autosave in the interface"):
  1.5 s after the last edit, toggle `#autosave` (localStorage `gt-autosave`). The stale-tab
  hazard (a tab that predates a migration saving over it) is now caught: PUT carries
  `X-Album-Base` = FNV-1a of the file as the tab read/wrote it, `src/main.rs` returns 409 on
  a mismatch, the tab stops autosaving and says so; ctrl+s after a conflict overwrites. The
  dev server must be restarted for a `main.rs` change (`cargo run`).
- **The system of unlocks** (owner, 2026-11-15): "opacity, line (existence of), root
  position motion, line length/bend/angle, shape (existence of), shape size/fill/morph
  (between square and circle), color … the idea of there just being dots, lines, and shapes
  is cool. lines and shapes can coexist. one level will have something like clocks with a
  circle and a line rotating inside of it. shape default unlock should be 100% circle, 0%
  square. oh i also want shape flatness. shape rotation maybe a separate unlock too? it's
  kinda nice to have a ton of minor unlocks and players might not even realize they've
  unlocked something". Then: opacity = glow (kept the name `glow`); "lines" stays the
  unlock name; lines point anywhere from the start, the `angle` unlock is motion ("just like
  dots can appear or not even without opacity unlocked"); "ring" not "shape"; "color not
  color"; "the order shouldn't really matter, you can just build everything". Built: twelve
  unlocks — bits 0–5 are the old ones renamed (rotation → `angle`, circles → `rings`), bend,
  size, fill, morph, flatness, rotation appended; `Shape::Line` has `bend`, `Shape::Ring`
  has fill/square/flat/angle and draws as a 48-gon; the `rings` kind gates each param at its
  default until its unlock (`gated` off = scenery). Not built: root position motion.
- **Glow is only a param** (owner, 2026-11-06): "i don't want the glow of everything to
  default to the pulse, i want glow to just be sidechainable but require glow unlocked to do
  anything" — a glow layer has one param, `level` (brightness 0..1, default 1), gated by the
  glow unlock's reveal; the old modes (field/inverse/bounded/off) and the engine's default
  field-on-the-pulse are gone. Cells under no glow layer stay lit. A draw layer's `glow`
  override is now an optional param of its own ("own glow"). Migrated: modes → base + depth
  on the existing pulse chain; act-1 got an explicit `field glow` layer (0.35 + 0.65 pulse)
  so its look didn't change — delete it to stop the field breathing.
- **Pitch** (owner, 2026-11-06): "is there a way to include like pitch of sections in the
  output too somehow? it sucks when there's a melody playing and it's not reflected in any
  way" — chosen: live, from the spectrum ("1 for sure"). Every band also gives `id.pitch`
  (loudest bin in its range, parabolic interpolation, log position lo..hi, glides with the
  attack, holds while the level is under the band's new `floor`) and `id.chroma` (that
  frequency folded to a pitch class, C = 0 .. B = 11/12). Caveat: over a mix the peak is the
  loudest *thing* in the range, not necessarily the melody. The exact route (MIDI from
  Ableton → a `notes` lane, `note`/`note.on`/piano-roll regions) is offered for later.
- **Stretches and stages** (owner, 2026-11-05): "it's probably fine to have a stretch of a
  track last as long as it takes to get through a few scenes, and then just loop at the end
  waiting for the trigger into the next part" — so a scene is a stretch (`bars`, `loop`)
  with `stages`; `Action::Stage` moves between them without touching the music. The first
  act became one stretch, `sine/act-1` (0–72, loop from 64), five stages. Also: "lock layer
  and hotspot durations to the scene by default and maybe have an unlock option" — no
  `bars` on a layer/hotspot means the whole scene; the inspector's "own range" unlocks it.
- **Nothing sinusoidal** (owner, 2026-11-05): music is events — each bar/beat/subdivision
  hits and falls away (square, or a maximally slanted sawtooth), never a sine. `hit(phase,
  fall)` and `pulse(modifiers)` in `scenes/mod.rs` are the envelope (beat hits, downbeat
  hardest); glow and anything else that pulses go through them. Smoothing is fine; tying
  parameters to the music's own bands (bass for the kick) is the direction.
- Web Audio throws on any time < 0: the first pass used to schedule its 20 ms lead-in before
  context time 0, fail silently, and be skipped — "no music for 10+ s" (until pass 2). The
  lead-in is clamped to 0 now and scheduling failures are logged to the console.
- Loading: tracks are fetched as a stream so the bytes count up (`audio.progress`); the view
  draws a bar along the bottom until the current track is decoded. First click only starts
  the audio context; music starts once the track is decoded (~250 ms after the bytes).
- Album order for now: sine → sasha grips → pisser → for.against → sine (sine first because
  pisser opens with 8 bars of near silence; agent's call, owner to reorder).

## Structural fact worth remembering

One line per cell can draw at most one orientation per cell, so a closed rectangle of
*missing* lines can't be contiguous in a field of parallel lines. The box puzzle works by
making the field point away from the box and the box's edge cells point inward (doubling
onto their neighbours), which leaves a contiguous blank band around the box.

## Ideas not done

- Canvas is fixed at 20 cells × 40 px = 800 px; doesn't fit phones or resize. Scale
  `SCALING` to `min(w, h) / 20` and handle `resized` when the owner wants phone-first.
- Free-form path matching for "draw" gestures beyond `Outline`.
- Per-song unique unlocks; more colors as more unlocks.

## The lattice font and the `text` kind (2026-11-19)

Asked what `words.rs` hinted at, the owner wanted letters: "letters will have to live in the space
between cells, so the dots are the vertices of the area that a letter is in. pixelated but sleek
… block letters might be too aggressive but it can't be like helvetica", then "smaller letters,
like an entire letter fits between its 4 vertices, cause otherwise we just can't write shit".
They drew the alphabet themselves on a connect-the-dots page (the artifact "Lattice Letters":
one cell, a grid of points inside it, click two points for a stroke, both cases, live previews
on the eleven-grid at a 33 px and a 72 px cell, the table as JSON). Decisions along the way:
a centre column was needed for m t v w and the i ("center all of them around that gridline"),
so the grid went 5 → 9 across; a and e were two units tall and "too big", so 7 → 13 rows and
they were redrawn at the x-height with a crossbar; 2 px looked best on desktop but the owner
chose **3 px at 150 %** ("make it 3px but just literally make the letters bigger") — the
piece's weight, letters spanning 1.5 of the box's grid (they only use the middle: ~0.375 box
wide, x-height 2/12 × 1.5 = a quarter box; one grid unit is 8 px on a phone, which is where
the lowercase first became legible). Lowercase is small (x-height two units of twelve),
capitals three; descenders reach the box's bottom dot row at 1.5.

- `tools/font.json` is the source (the page's table, verbatim, plus w/h 1.5 and stroke 3);
  `tools/font.py` writes `app/src/scenes/font.rs` (COLS/ROWS/WIDTH/HEIGHT/CENTER, GLYPHS sorted
  by char, `glyph(c)` with the other case as fallback, `shift()` for centring). Rerun it after
  editing the JSON; the page can load a pasted table ("load what's pasted below").
- `Kind::Text` in data.rs: `TextLayout::new` at install lays the text into the region's
  rectangle — boxes are one fewer than dots each way (11 dots → 10 letters a line), the first
  line's boxes have their bottom-left dot at `j1 − 2`, lines go down `lead` rows, words wrap,
  a word longer than the line is cut, a newline breaks; `align` left/center/right; `center`
  shifts each glyph to its box's middle; `reveal` 0..1 shows the first ceil(reveal × n)
  letters. The render arm draws each stroke with `stroke()` from the box's bottom-left dot
  (x, y) — px = 0.5 + (g/COLS − 0.5)·width, py = (1 − height)/2 + g/ROWS·height, in units
  of the cell — so the piece's 3 px weight and round caps apply, and morphs work.
- The editor got a `string` param type (`multiline` → a textarea).
- Open: `random_words`/`paired_words` as a source for `text` (a word that changes per visit),
  words as hotspots, the hint text "met ends" redrawn in the font.

## Named exits (2026-11-19)

Owner: "i want every room to have up to 3 (maybe 4 for the occasional list of 4 i have?)
exits, each of which gets a name from the list of words, which is randomized at the beginning
of every game but persists through saves and transitions, and i'd like the preview to have a
button to reset names. but so in the editor i would give hotspots 'name 1' 'name 2' 'name 3'
if i wanted a random one, or i could write my own".

- `words.rs` is compiled in at last (`random_words` fixed to a flat `&[&str]`; the file had
  it typed as a list of lists and had never been built). `names.rs`: seed in localStorage
  `gender-theory/names` (made on first use from `Math.random`), one Fisher–Yates shuffle
  (xorshift64*) per seed, `exit(room, k)` = `random_words[order[room × 4 + k]]`, so no two
  exits share a word; `reset()` new seed, `clear()` with `?reset`; `resolve(name, room)`
  turns "name N" into the word. Rooms = the album's data scenes in install order (so
  inserting a scene shifts later rooms' words — fine, they're random; the seed is what
  persists). wasm exports `reset_names()` and `exit_name(room, k)` for the editor.
- The name is said by a **text layer** with `hotspot: <id>` (owner: "tie a text to a
  hotspot instead of the setting being in the hotspot? that way i can place the text where i
  want" — it was `label: true` on the hotspot for an hour, drawn in its rectangle). At install
  the layer's content is `names::resolve(hotspot.name, room)` and `LayerRt.hotspot` is the
  hotspot's tag; `render()` skips the layer unless `model.hint_visible(tag)` (needs met, not
  spent, right stage). Every other text param applies (region, align, size, reveal,
  brightness, color), so a name can fade in with a sidechain or type itself out. An id that
  matches no hotspot falls back to `text`. A word needs letters + 1 dots across.
- Editor: "reset names" in the top bar (new seed, then the album is re-pushed so layouts
  rebuild), the resolved word shown beside a "name N" field and beside a text layer's
  `hotspot` picker (one of the scene's hotspots, or —).
- Migration 2026-11-19: the four labelled hotspots in `sine/1` became text layers named
  "name: top/left/fill/outline" (the owner's empty foyer text layer was wired to "top";
  the others got the hotspot's own region, centred) — placeholders for the owner to move.

## Holes, and the bars palindrome (2026-11-19)

- Owner: "i marked hotspots in the 'left' stage, and i want the lines to be invisible as they
  pass over those hotspots (not sure how to add that as layers)". The `hole` kind: a cell's
  `render()` notes `model.marks_len()` at its start, and a hole layer calls
  `Model::cut_marks(first_mark, keep_dots)` — the scenery marks the earlier layers recorded
  for this cell go (lines and rings; dots = zero-length lines stay unless `dots` is off) —
  and zeroes the cells rows' lines and rings (brightness too without dots), since a cells
  layer's own mark is only drawn after the kinds loop. Later layers draw over it. With
  `hotspot` set the layer's region is replaced at install by that hotspot's region and the
  hole is there only while the hotspot shows (`LayerRt.hotspot`, like a text layer saying
  its name). A segment owned by a dot outside the hole still reaches the hole's edge dot —
  the lines touch the hole; that's the look, not a bug.
- Added two hole layers to `sine/1` stage left for the owner's two hotspots there.
- bars: "extend from the same side it just retracted to after the extend, rotate, retract
  loop, but in opposite directions of what it just did (no pause still)" — that is the first
  three parts played backwards: `s = t` for t < 3, `6 − t` after; the parity flip is gone.
  Rows grow again out of the dots they retreated into, the pinwheel turns back, the columns
  retreat into their origin dots, and t = 6 meets t = 0 without a seam.
- `Action::All(&[])` (a hotspot with no action yet) used to count as spent, so such a
  hotspot never showed — its name and hole with it. Now an empty action is not spent.
- Owner (2026-11-20): "there shouldn't be vertical holes in the horizontal bars and vice
  versa … the holes should only be there when the animation is going in the direction of
  the hole". A hole has a `direction` (auto from its bounds: wider → horizontal, taller →
  vertical, square → any; or set). Directional: a line is scaled by 1 − cos²(angle − dir)
  — along the hole it goes, across it it stays, the pinwheel's turn dims it smoothly rather
  than popping at 45° — rings stay; `cut_marks` dims by multiplying the mark's color; for
  the cells rows, `lines` is scaled by the same factor from the angle row. `any` cuts all
  as before. Stored on `LayerRt.direction` at install.
- Owner (2026-11-20): "i probably described it wrong before, but i want the lines to switch
  direction and also switch odd/even so it's like it's coming from the other side in each
  line". bars now: second half = first half backwards *and* parity-swapped (`s = 6 − t`,
  `flipped = true`): rows regrow from the other end of each line, turn back, columns retreat
  into the far ends; the next cycle's columns then come from the other end too. The halves
  meet on an empty grid, so no seam either way.

## bars, third time (2026-11-20)

Owner: "the scene is actually — there's a vertical line missing and the bars just bounce back
and forth vertically …, and then outlining it unlocks angle, which adds a turn animation …
instead of having a turn separate from everything else, just have them all turn 90 degrees
over like one beat in the same direction they're doing now and start the next extend/retract
animation instantly. so then it can be cycles of 4 bars"; "line length also has to be
unlocked for those parts of the animation to work".

- Four parts of `per` beats: extend, retreat, extend from the other end, retreat. Without
  angle the columns just bounce. With angle (`model.reveal(Unlock::Angle)`): over the first
  beat of each retreat every segment turns `sense` (−90° if col parity == row parity, else
  +90°: the pinwheel, rows tiling) while the retreat already runs along the turned way; the
  next extend is from the other side of that line (pointing `vertical − sense`), the next
  turn is the same sense again and lands vertical; `vertical` flips every other cycle so
  the columns too come from the other end — period 8 bars. The fill is counted from the
  root of `pointing` (the way the segment points: up → from the bottom, etc.), which keeps
  every retreat the reverse of a growth (tips move smoothly).
- Gates, in the bars kind itself since rows don't reach it: segment length × reveal(Lines);
  fill = 1 + (moving − 1) × reveal(Length) — without length every segment is a full pitch,
  the motion eases in; the turn × reveal(Angle). The retreat *order* switches from
  vertical to horizontal the moment angle is unlocked (one discrete change, at the
  gesture); the turn itself eases in over the reveal.
- Owner (2026-11-20): "the hotspots should be named after the stage they send you to, if
  they send you to a stage". `label_of(hotspot, room)` in data.rs: a `stage` action names
  it, else `names::resolve`. The editor's hint beside the name says so; the name field is
  then just a label for the lists.

## Painted bases (2026-11-20)

Owner: "can i get an editor where i can click cells and edit and copy their settings … i want to
be able to edit complicated line patterns without having to make 4 or more layers for different
line orientations". The answer that keeps the layer design: a row's base can vary per cell.

- `Param.grid: Option<Grid>` (`data.rs`): `{map, legend}` — a region-style map of glyphs,
  `legend` glyph → value. In the file a base is a number or that object (`BaseJson`,
  untagged), on params and rows alike. `Param::base_at(i, j)` reads the cell; `painted(i, j)`
  is false for `.` / unknown glyphs and the cells loop skips the row there (the cell is left
  as the layers before left it; the layer's other rows still apply). Chains and masks sit on
  top as before, so one layer = one pattern + its motion + its gate.
- Editor: `baseInput` (paint / painting / flatten), `paint` state, `painting()`, `paintCell`
  (converts a non-map region to a map when painting outside it), `paintDown/Move/Up`,
  `drawPaint`, `paintTools`. Glyphs: arrows for multiples of 45° on angle/rotation, else
  `a-z A-Z 0-9`, then box-drawing chars. `flatten` → the commonest value. Stamp = shift-drag
  a block, click where the drag began should land.
- Not done yet (offered): the cell inspector — click a cell, see the stack of layers at it
  and edit/copy across rows.
- Test: scratchpad `cdp_grid.py` (engine frame + editor driven by real clicks with autosave
  off and PUTs failed through `Fetch.enable` — never let a headless editor save the album).
- Python patch scripts go through the Write tool, not a bash heredoc: a heredoc script with
  non-ASCII (`·`, arrows) fails to match the file.

## dots is a row (2026-11-20)

Owner: "dots should just be treated like other unlocks for simplicity. that way i can disable
dots on some cells". `Unlock::Dots` (last in the enum — the progress bits in localStorage are
by discriminant — second in `ALL`, after glow), row `dots` after brightness in `ROWS`:
start 1, `Gate::Sidechains` (free from the start; the unlock is the dot's own motion). The
dot is the line of no length, so dots only shows where there is no line: the cell's mark is
drawn at brightness × dots when `line` is 0, at brightness otherwise, and skipped when both
are nothing (rings still draw). The hole kind's indices are now `ROW_*` consts. Thirteen rows
everywhere the docs counted twelve.
- Adding a row touches: `Unlock` (enum, `COUNT`, `ALL`, `name`), `ROWS`, the `Cells` struct
  **and its `row(k)` list** (missed the first time: `?editor` loads panicked "len is 12 but
  the index is 12" and the page stayed on the old album — a panic in install leaves the game
  dead, so always run a headless frame before committing a build), the destructure in the
  cells drawing, `ROW_*` consts, the editor's `UNLOCKS`, and the counts in AGENTS/SPEC.

## Stage names are random words (2026-11-21)

Owner: "it's the stages that i want to have random names, all of them actually … make all
hotspots that lead to stages: 1. show their name after the destination stage has been visited
once 2. use a random name that's fixed for the duration of that save file 3. … linking a text
layer to a hotspot 4. be centered horizontally in the available space". Supersedes the
2026-11-20 "named after the stage" note.

- `names::stage(room, s)` = word `len − 1 − (room × 16 + s)` of the same shuffle — from the
  far end, so the exits' words (from the start) never collide. Same seed, same resets.
- `progress.rs` saves `scene unlocks [id bits]…`: `Visited = HashMap<scene id, u64>`, a bit
  per stage. `Model::note_visited()` runs every update after the landing/seek block (and in
  `new`), so every way the stage gets set counts — landing, the editor's stage picker, a seek.
  Nothing is saved in `?editor` mode, but the in-memory bits still make viewed stages' names
  show in the editor. Old saves without the third field load fine (no stages visited).
- `LayerRt.to_stage`: a text layer saying a stage hotspot's name skips drawing until
  `model.stage_visited(stage)` (checked on the *viewed* scene, so morph previews are right).
  `TextLayout::new(…, centred)` ignores `align` for those and shifts by half a box
  (`font::COLS / (2 × width)` grid points) when the spare boxes are odd — the only true
  centring in the text kind; plain `align: center` still rounds down to a box.
- Editor UI unchanged (owner: "no need to change anything in the ui"); only the hint beside a
  stage hotspot's name now shows the word (`stage_name` export) and says when it shows.

## + song, rows on one line (2026-11-21)

- Owner: the "nothing for the masks to land" readout note flickered on a pulse sidechain and
  rewrapped the row; "all of the row settings should be in a row, even if we need horizontal
  scroll". `.row`/`.chain`/`.fparam` are `flex-wrap: nowrap`, children `flex: none`, the
  doc the one flexible item (wrapping in its own column; a row's doc single-line with
  ellipsis + title), readouts a fixed width; the note is now "· masks idle" with the sentence
  in the tooltip. `#inspector` already scrolls both ways.
- Owner: "add the upload button! i want people to be able to try it with their own music".
  `addSong(file)` in editor.js: decode in the page, `guessTempo` (phases.py's method in JS:
  10 ms log-energy onsets, autocorrelation peaks 60–200 bpm, best beat-grid fit ±2 bpm),
  a track `{id: <file stem>, path: music/<file name>, bpm, scenes: [one over all the bars]}`;
  `uploads[path]` = object URL, and `wire()` swaps it in whenever the album goes to the
  engine (`push`, seeks) — `livePath` for the waveform fetch. Tab-only: after a reload the
  path 404s and the status says to pick it again. The **bpm** field in the top bar edits any
  track's tempo (new; there was no way to).

## every / offset on a chain (2026-11-21)

Owner: "if i want something to happen every 2 beats? or 3?". `Sidechain.every` (default 1) and
`offset` (0): `Signals::get_timed` — for beat / bar / beat.phase / bar.phase, a cycle of n
beats (bars) from the downbeat, running across bars (`bars × 4` for beats), `offset` in; the
hit falls over one beat as before (`hit(phase × max(n, 1))`), the phases run over the cycle;
`delay` still shifts in cycles. Other sources ignore both. The editor's chain meter mirrors it
(`chainLevel`). Fields' parameters go through `level()` too, so a field's phase can be on a
3-beat cycle. Pulse untouched (it is the bar's shape).

## Delay unlocks (2026-11-21)

Owner: "what if every stage started without unlocks so the effects of the unlocks were more
obvious? … a setting per stage … 'delay unlocks' … literally turn off the unlocks so it affects
hotspots too". `SceneData.unlock_delays: {stage name: bars}` (`""` for a stageless scene) →
`Scene::delays: &[f32]` per stage (hand-written scenes: empty). `Model.at` notices a change
of (scene, stage) in `note_visited` (every update, so landing, the editor's stage picker and
seeks all count) and sets `hold = (time, bars × 240 / bpm)`; `hold_in()` is the smoothstep
of it, 0 on the morph's landing frame of such a stage (preview), 1 when nothing holds;
`reveal(u)` is `min`'d with it and `visible(hotspot)` needs `hold_in() == 1` for a hotspot
with any `needs`. The editor: a number per stage row (and one for a stageless scene) in the
stages fieldset; rename/remove keep the map in step.

## delay → ripple (2026-11-21)

Owner read the chain's `delay` as a plain delay ("setting the delay makes the cells desync
from each other"); it is the ripple (× distance). Renamed `ripple` — `#[serde(alias =
"delay")]` in Rust, `migrate()` in the editor on load (walks every object with `source` +
`delay`), so album.json converts on the owner's next save without a disk rewrite (a rewrite
would 409 their open tab). A uniform shift is `offset` (beats, fractional).

## The wasm must be versioned with the glue (2026-11-21)

After a deploy the owner's browser paired the new `gender.js` (busted `?v=`) with the old
`gender_bg.wasm` it had cached (`/app/` expires 1h; the glue fetches the wasm by a fixed URL):
`LinkError … __wbindgen_closure_wrapper… function import requires a callable`, blank page,
fine in incognito. `index.html` now has `const glue = './app/gender.js?v=dev'` and inits with
`new URL(glue.replace('gender.js', 'gender_bg.wasm'))` — ops rewrites that line's `?v=`.
The editor already used `?v=${Date.now()}` for the wasm. Also: a headless profile reused
across a deploy shows the same stale pairing — use a fresh `--user-data-dir` per run.

## Group presets, curve 0, negative length (2026-11-21)

Owner: "is it possible to redo bars as a group of normal layers instead of as a special thing"
→ "i like the idea of preset groups"; and, for later: "save people's mp3s and their levels and
their groups, so people using the editor can browse and rate groups … upload their levels".
- Presets: `{id, name, layers: [...]}` beside `{id, name, layer}`; `saveGroupPreset` (group
  header), `insertPreset` handles both (a fresh group name per drop). Keep the group a
  self-contained JSON document — it is the sharing unit when the server side comes.
- `curve: 0` on a chain = straight ramp (engine `shape()` and the editor's copy). Needed
  because the bars kind grows linearly and the smooth step can't be undone.
- `length < 0` draws the line the other way out of the dot (angle + 180, |length|) — so a
  dot can draw its pitch backward: the bars' part 2 ("extend from the other end") without
  swapping which dot owns the pitch.
- The bounce as layers (angle locked): two cells layers, `cols [2, 0]` and `cols [2, 1]`,
  lines 1, angle 90 / 270, length = curtain A (depth +1, turn down/up, edge 1, offset base
  5.5 with bar.phase-every-4 chains depth −11 [0, .25] and +11 [.25, .5], curve 0) + curtain B
  (depth −1, the other way, quarters [.5, .75] / [.75, 1]). The pinwheel (angle unlocked) is
  not done as layers yet: needs the plain pair masked by `unlock:angle` inverted and an
  angled set of 4 (col parity × checker sense) masked by `unlock:angle`.
- Headless drivers must import the page's own module instance: `import('/app/gender.js?v=dev')`
  (index.html's `const glue` string), not `/app/gender.js` — a different URL is a second,
  uninitialised module and every call throws `__wbindgen_add_to_stack_pointer`.
- The bounce group lives in album.json as the preset "bounce" (two layers, `cols [2, 0]` /
  `[2, 1]`, regions unbounded — set a rect on both layers to confine it).

## The pinwheel as layers; the `bars` kind removed (2026-11-22)

Owner: "can we do the pinwheel? and remove the bars kind." The `bounce` preset is now six cells
layers, generated by `tools/bounce.py` (prints the group): the plain pair as before, plus four
pinwheel layers (`cols [2,a]` + `rows [2,b]`), masked by `unlock:angle` on their angle and
length rows (a crossfade from the plain pair: masked `set` is `cell += (value − cell) × amount`).
- Angle: base V (90 even cols, 270 odd), s = −90 if a == b else 90 (the pinwheel sense), and
  the drawn angle kept within 90° of V by letting the length's sign absorb the half turns:
  chains on `bar.phase every 8`: ramp s over the first beat of bar 1 (threshold 1/8, width
  1/32), step −180 at 3/8, ramp s at 3/8, ramp s at 5/8, step −180 at 6/8, ramp s at 7/8
  (back to V at the wrap). A step's width is 0.0002 of the phase (the fill is 0 at every bar
  boundary, so steps there are invisible).
- Length: eight one-bar curtains (`turn` = the direction toward the line's root, i.e.
  pointing + 180°; `turn_of(deg) = ((90 − deg)/360) mod 1`): grow = offset 5.5 → −5.5 over the
  bar then a step back; retreat = a step to −5.5 at the bar then a ramp to 5.5. Bars 0..7:
  grow V / retreat H / grow H+180 / retreat V / grow V+180 / retreat H+180 / grow H / retreat
  V+180 (H = V + s), depths + + − + − − − − so that where the two states point opposite ways
  the crossfade passes through the dot instead of swinging 180°.
- Verified headless (`cdp_pinwheel.py`): one scene, two stages (the kind / the group), paused
  at 14 moments across the 8 bars, screenshots XORed: 1–3 % of lit pixels differ, except in
  the first ~3 % of each bar (the curtain's smooth edge vs the kind's linear segment growth:
  a ~35 % XOR of a few hundred pixels — the same easing difference the plain bounce has).
- Then the kind went: `Bars` struct/Default/arm/parse/`CATALOGUE` entry (`patch_nobars.py`),
  132 lines. Nothing in the album used it. The hole doc lost "bars" from its scenery list.
- Partial reveal (the 2.5 s or a stage's delay-unlocks hold-in): the kind eased each quarter
  turn by the reveal; the layers crossfade plain↔pinwheel — a ≤ 90° swing and lengths through
  zero where the directions are opposite. Different, not worse; the owner hasn't seen it.
- Album stats at the time (for the unlock-semantics question): grants glow 2, angle 2, lines 1,
  length 1; needs glow 1, lines 1; masks unlock:glow 2, unlock:lines 3, unlock:angle 2; rows
  used brightness 54, angle 51, length 45, color 38, lines 31, dots 1.

## Unlocks as set/move pairs; a layer's `needs` (2026-11-22)

Owner: "formalize the unlocks actually unlocking things so i don't have to remember to put a
mask for every unlock … an unlock is either the ability to set a value, or the ability to
automate it … the purist in me still wants to stick with 1" → "yeah" to the table.
- `RowSpec { set: Option<(Unlock, f32)>, motion: Option<Unlock> }` replaces `unlock` + `Gate`.
  brightness: set Dots (0) / move Glow; dots row: free; lines: set Lines (0); length: move
  Length (base free — it used to rest at 1); angle: set Angle (0) / move Rotation; bend, rings,
  fill, morph, flatness, color: set; radius: move Size (used to rest at 0.4); rotation row: free.
  The fold eases a set row from its rest by the reveal — the angle the short way round
  (`(delta + 180).rem_euclid(360) − 180`).
- The Angle bit now means "set the angle"; the Rotation bit "move it" (spin + music chains).
  Bit values unchanged (progress holds); `Unlock::ALL` display order: dots, glow, lines,
  length, angle, rotation, bend, rings, size, fill, morph, flatness, color.
- `Layer.needs: Vec<String>` → `LayerRt.needs: Unlocks`; `need` = min reveal over them; cells
  rows' `amount *= need` (crossfade over the layers before), snake/text/orbits brightness
  × need, hole skipped under 0.5. Editor: a `needs` checkbox row in the layer inspector; the
  old `delete l.needs` migration line is gone.
- Album `version` 2: `migrate()` renames `angle` → `rotation` in hotspot grants (`{unlock}`
  objects with that one key), `needs`, and `unlock:angle` sources, once (`album.version < 2`).
  album.json migrated on disk by `migrate_album.py` (2 grants, 10 masks) and the bounce
  preset regenerated with `needs: ["rotation"]` instead of masks (`tools/bounce.py`).
- Consequence the owner accepted: everything is dark until a `dots` unlock. The album has no
  dots door yet — the live-site deploy entry stays at `66f99f6` (pre-change) until it does.
  In the editor tick `dots` in the unlock bar.
- Verified headless (`cdp_unlocks.py`): nothing → 0 lit px; dots → dots only; +lines →
  horizontal half-pitch lines (angle rests 0, length base 0.5 lands); +angle → 60°; +length →
  pulsing with the beat chain; +rotation → spin. Preset: columns without rotation, rows with.

## Scene and track order, copying stages, moving across tracks (2026-11-22)

Owner: "i feel like i should be able to re-order scenes and copy stages between them and stuff".
Editor only (no engine change):
- `nudgeSelected` takes scenes too (↑/↓ in the inspector head); `si` follows. Order matters to
  the engine as the album's order of rooms: `names::exit/stage` number rooms in it, and
  `scenes::all().first()` — the first scene of the first track — is where the game starts.
  `moveTrack(d)` (◂/▸ beside the track picker) swaps tracks for the same reason.
- `copyStage(s, k, to)`: the stage's layers and hotspots deep-copied, hotspots with fresh ids
  and references remapped (`hotspot` params, `hotspot:` sources), stage name made unique in
  the target (`name`, `name 2` …), `unlock_delays` carried, `bars` dropped when crossing scenes,
  `then: {stage: name}` → the new name; other stage actions kept, listed in the status if the
  target has no such stage. Stageless layers are not copied (they are the scene's base).
- `moveSelectedTo` / `moveRow` work across tracks (`findScene(id)`, `sceneOptions()` with an
  optgroup per track); an item's `stage` is dropped if the target lacks it.
- Verified headless (`cdp_order.py`, `cdp_order2.py`: editor.html with autosave off and PUTs
  failed through `Fetch.enable`): scene ↑/↓ and track ◂/▸ reorder and undo; "bottom" copied
  twice into pisser/1 → "bottom" and "bottom 2", 4 layers + 3 hotspots each, delay 1 carried,
  7 distinct hotspot ids in the scene, both holes resolving to the new horizontal/vertical.
- Reusing a headless Edge for a second CDP script after the first exited hung every evaluate
  (no replies at all); a fresh profile per script run is the reliable way.

## The bars scene: the bounce as rows (2026-11-22)

Owner moved the first act's "bottom" stage into a new scene `bars` (sine, bars 16-32, one
stage `bars`) and put its "turning bars" layer (every row of lines, even rows pointing right,
odd rows left) in a group `bars`: "i want the bars effect in normal layers ... flesh it out".
`tools/bounce.py` grew `rows=True` (`--rows`): the same six layers transposed - even/odd rows,
the plain angle 0/180 (the owner's map), the pinwheel by row parity x column parity. Inserted
into the group after "turning bars" and before the scene's two holes (a hole cuts what is
before it). The columns preset is unchanged (0 differing leaves; only key order differs in
the file). Verified headless (`cdp_bars_scene.py`, `?scene=bars`, `editor({play:true})`):
dots+lines+angle -> 968 lit px (dots: the bounce sets length 0 everywhere, so "turning bars"
shows nothing on its own); +length -> horizontal runs 2x vertical through the cycle;
+rotation -> bars 1-2 of 8 flip to vertical runs 2x horizontal. Without `angle` every line
folds to 0 deg (all horizontal) - the scene needs the angle unlock as well as length.
Loose end for the owner: the stage's "name 1" hotspot still says `stage: foyer`, which this
scene has no stage of (it was a way back in sine/1).

## The curtain's pause at each cell (2026-11-23)

Owner: "a slight pause at each cell which makes it count in like 11/4 ... can the animation be
smoother". Cause: two smooth steps stacked - the curtain field's `inside` (edge 1 cell) and the
position chain's `curve 1` on top - so the front's speed hits 0 at every cell (0 / 2.25x mean).
Computed over a bar (sum of the cells' lengths vs offset): edge 1.5 + chain curve 0 runs at
0.89..1.11x mean with the front still under a cell wide; edge 2 curve 1 the same but 1.2 cells
wide; edge 3 the flattest (1.8 cells). `tools/bounce.py`: EDGE 1.5, CURVE 0, and the curtain
rests at REST = 5 + EDGE/2 (5.75) travelling TRAVEL = 2 x REST, so the wider edge is wholly off
the grid at rest - at 5.5 it left 7% stubs on the end cells (lit 1320 instead of 968 px at
rest). Applied to the bars scene's six layers; the album's columns preset still has edge 1 /
curve 1 for comparison. Verified headless: rest 968 px, bounce and pinwheel as before.

## No hole kind; the `along` field; new layers start empty (2026-11-23)

Owner: "i don't want brightness as a default row. also i don't want the huge block of text
next to the needs ... i want to get rid of the holes kind, but i'm not sure how to code the
line values to sync with the direction of the lines".
- The sync is a mask source that reads the cell's own angle: `Signals.angle` (the angle row as
  the layers before left it, set per layer in the renderer loop) and a field `along`
  (`family: "line"`, param `direction` deg) = cos^2(angle - direction). A door's hole is a
  cells layer: `lines` multiply 0, masks `hotspot:<id>` x `along <direction>` - the same
  numbers the hole kind used (`lines *= 1 - cos^2`), `rings` x 0 for a square one,
  `brightness` x 0 for dots off. Position chains are fixed sources, so it acts under any unlocks.
- Removed: `Hole` struct, `Kind::Hole`, the catalogue entry, `LayerRt.direction`,
  `Model::cut_marks`, `ROW_LINES/ROW_RINGS/ROW_BRIGHTNESS` (only `ROW_ANGLE` stays). Scenery
  (snakes, letters) under a door can no longer be cut - accepted with the kind.
- Editor: `defaultParams` for a rows kind is `{}` (a new scene's `field` keeps brightness 1);
  the `needs` doc span is gone; `migrate()` version 3 turns hole layers into the cells recipe
  (direction from `direction` or the map's shape). The album had no holes left (the owner had
  already deleted the bars scene's two).

## One unlock for the angle (2026-11-23)

Owner: "right now you basically unlock angles twice which conceptually sucks". The angle row:
`set: None, motion: Some(Angle)` - the base is free like length's, `angle` makes it turn (spin,
music). `rotation` is the ring's rotation row's motion again (`motion: Some(Rotation)`, was
free). Bits unchanged; `Unlock::ALL` order dots, glow, lines, length, angle, bend, rings, size,
rotation, fill, morph, flatness, color (editor UNLOCKS the same). Album `version` 4: grants,
needs and `unlock:rotation` masks -> angle (they all meant the lines); `tools/bounce.py`
pinwheel `needs: ["angle"]`. Consequence: a painted angle shows before any unlock (lines
unlock shows the lines as painted, not horizontal).

## Stages in the scene list (2026-11-23)

Owner: "let's just put the stages in the same browser thing as the scenes with the same play
button ... and get rid of the dropdown". `#scene-list` renders each scene's stages as
`li.stage-item` under it (`data-index`, `data-stage`; `layers.hotspots` counts at the right);
the stage's play button sends `{stage: k}` if the scene is playing, else `{seek: {scene, bar,
stage: k}}` - the engine's seek carries an optional stage (`Model.seek: Option<(String,
usize, Option<usize>)>`), landing in it instead of 0. Clicking a stage sets `viewStage`
(cleared when the edited scene changes), which `viewedStage()` prefers over the playing
stage; `#stage-pick` and the inspector's per-stage "view" button are gone. The playing
highlight is per scene and stage (`lastPlayingKey`).
- Follow-up: the editor jumps to whichever scene starts playing (on a change of `st.scene`:
  across tracks too, `renderAll()` then), and a scene picked by hand sticks until the next
  change; the `follow` flag is gone. Owner: "it should probably take you to the scene that
  starts playing when you switch". The playing scene/stage marks are set as the list is built.

## Done hotspots: waves and `after` (2026-11-23)

Owner, for the bars scene: "drawing the horizontal line to send out a ripple synced to the
beat which freezes all the bars and turns them vertical as it passes over and erases the line
that needs to be drawn at the vertical hotspot, which unlocks that hotspot".
- `Model.done: Vec<Option<f32>>` by tag, the bar (next beat) a hotspot's gesture completed
  this visit; `complete(hotspot)` replaces the two `act(&hotspot.then)` call sites (release,
  outline closed); cleared in `enter()`. `done_at(tag)`.
- `Hotspot.after: &'static [u8]` (data: ids) - `visible()` waits for them; a hotspot with
  `Action::All(&[])` is hidden once done (a trigger). The 23 hand-written hotspots got
  `after: &[]`.
- Source `done:<id>` (handled first in `Signals::level`, fixed): `clamp((beats_since - d *
  every) / every, 0, 1)`, d = the cell's distance to the hotspot's nearest cell
  (`DataScene.dists`, 121 floats per hotspot, built with the scene). `every` = beats per cell.
- Editor: `after` checkboxes + "done now" (`editor({done: k})`, k the hotspot's index) in the
  hotspot inspector; `done · name` sources; `st.done` for the meter; copyStage remaps `after`
  and `done:` sources.
- Bars scene: horizontal `then: []` (a trigger - it granted glow), vertical `after`
  horizontal, layers "frozen vertical" (angle set 90, length set 1, masked done:horizontal)
  and "where the vertical line goes" (the vertical's cells, lines x 0, same mask) at the end
  of the bars group. Frozen = full-height continuous columns; the owner may want another look.
- 2026-11-27: the chain rows' source `<select>` became `menuButton({tree, value, title, onpick})` + `sourceTree()` in editor.js (a native select can't nest optgroups): branches unfold on hover or click beside the row, flip left at the screen edge; `.menu-root` on body, one open at a time, Escape / outside click close it. Headless test `cdp_submenu.py` (editor.html; drives it with real mouse events via CDP). editor.js is a `type=module` script — `album`, `undo` aren't page globals; read the DOM or send ctrl+z. Undo clears the selection (`sel = null`) by design. The websocket module is only in `C:/Python312/python`.
