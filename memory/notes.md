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
- Taps land on the next bar. White border + input block from trigger to landing.
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
- **Painting is the default** (owner, 2026-11-06): "if i'm editing a hotspot or layer, i
  should be able to just click on the map. and default it to empty and 'map' setting instead
  of 'whole'" — new layers and hotspots start as an empty map and are painted on the grid;
  hotspots have a `region` like layers (`Hotspot.inside` narrows the bounding `cells`;
  outline/hints use the rectangle round it; old `rect` still reads). The drag-paint bug was
  an operator-precedence inversion in the grid's mousemove.
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
- Per-song unique unlocks; more colours as more unlocks.
