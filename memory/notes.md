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
  element wants it). Python's `http.server` doesn't do Range; a 30-line handler does.

## Tracks

The real music lives in `Desktop\gender theory\` (mp3 bounces + Ableton projects; the
project folder `gender theory\gender theory\` has the paired-word projects: being.useless,
for.against, porcelain.piss, terra.firmament, violence.release). Two bounce names contain
U+F022 where a dot should be (`for<U+F022>against.mp3`, `terra<U+F022>firmament.mp3`).
`static/music/` holds the copies the app uses — the owner's files, byte-identical:

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
- The first draw (owner's design): `sine/0` is stripes — every other row of points joined by
  a stripe, one stripe missing over 8 points that glow; draw through them → Lines, the
  points become horizontal lines and the stripes dissolve into them; then the door. A
  one-row `Area` with `Gesture::Outline` is "draw through these points".
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
