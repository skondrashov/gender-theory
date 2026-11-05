//! The album: the real tracks, cut into their phases. Each phase is a scene looping its
//! bars. The first act (the owner's design, on sine) teaches the first unlocks: glow,
//! lines, length. After that the phases are placeholders — one rendering varied a little
//! per phase, a centre tap leading on — waiting to be designed. The end of the album leads
//! back to its start. Transitions between phases are deliberately not done yet.
//!
//! The cuts come from `tools/phases.py` (see the bar maps in `memory/notes.md`).
//!
//! Lines, so far, hold still, point along the grid and are one cell long.

use crate::scenes::*;

pub static SINE: Track = Track {
	id: "sine",
	path: "music/sine.mp3",
	bpm: 135.0,
};

pub static SASHA_GRIPS: Track = Track {
	id: "sasha-grips",
	path: "music/sasha-grips.mp3",
	bpm: 147.0,
};

pub static PISSER: Track = Track {
	id: "pisser",
	path: "music/pisser.mp3",
	bpm: 120.0,
};

pub static FOR_AGAINST: Track = Track {
	id: "for-against",
	path: "music/for-against.mp3",
	bpm: 128.0,
};

/// The way on, in every placeholder phase.
const DOOR: Area = area(8, 8, 12, 12);

/// Sixteenths walk across the screen.
const SQUARES: Regions = Regions::Grid { cols: 4, rows: 4 };

// ---- the first act, on sine ----

/// The screen as text: `CANVAS_HEIGHT` rows of `CANVAS_WIDTH` characters, top row first,
/// whitespace ignored. What each character means is the scene's business.
fn at(map: &str, i: usize, j: usize) -> char {
	map.chars()
		.filter(|c| !c.is_whitespace())
		.nth((CANVAS_HEIGHT - 1 - j) * CANVAS_WIDTH + i)
		.unwrap_or('.')
}

/// "met ends", in points. Every letter is a line of points one point thick, except the e
/// in "met", which is blocky: its strokes are a whole unit thick. That's the one to tap.
const MET_ENDS: &str = "
	....................
	.#####.######.#####.
	.#.#.#.######...#...
	.#.#.#.##..##...#...
	.#.#.#.######...#...
	.#.#.#.######...#...
	.#.#.#.##.......#...
	.#.#.#.######...#...
	.#.#.#.######...#...
	....................
	....................
	####.####....#.####.
	#..#.#..#....#.#....
	####.#..#.####.####.
	#....#..#.#..#....#.
	####.#..#.####.####.
	....................
	....................
	....................
	....................
";
const E_IN_MET: Area = area(7, 11, 13, 19);

fn met_ends(draw: &Draw, model: &Model, i: usize, j: usize, _time: f32, modifiers: [f32; 8]) {
	let [_whole, _half, _quarter, _eighth, _sixteenth, _bass, _mid, _treble] = modifiers;
	let unit = model.scaling as f32;
	let (x, y) = scale_coordinates(model, i, j);
	if at(MET_ENDS, i, j) == '#' {
		mark(draw, model, x, y, 0.0, 0.0, unit, 0.9 * glow(model, pulse(&modifiers), false), 0.0);
	}
}

/// A square. Tap it and everything glows; a second square glows the other way round,
/// black at the peak. Tap that.
const SQUARE: Area = area(3, 7, 9, 13);
const SQUARE_2: Area = area(12, 4, 17, 9);

fn square(draw: &Draw, model: &Model, i: usize, j: usize, _time: f32, modifiers: [f32; 8]) {
	let [_whole, _half, _quarter, _eighth, _sixteenth, _bass, _mid, _treble] = modifiers;
	let unit = model.scaling as f32;
	let (x, y) = scale_coordinates(model, i, j);
	let field = model.reveal(Unlock::Glow);
	let gray = match model.hint_at(i, j) {
		// The square, until it's been tapped.
		Some(1) if SQUARE.on_perimeter(i, j) => 0.9,
		Some(1) => 0.0,
		// The second square: the inverse.
		Some(0) => glow(model, pulse(&modifiers), true),
		_ => 0.9 * field * glow(model, pulse(&modifiers), false),
	};
	mark(draw, model, x, y, 0.0, 0.0, unit, gray, 0.0);
}

/// A patch glowing the other way round; tap it for lines. Then the field is stripes —
/// every other row of points joined up — with a gap in one where the points stay bare.
/// Draw through them.
const PATCH: Area = area(7, 7, 13, 13);
const GAP: Area = area(6, 10, 14, 11);

fn stripes(draw: &Draw, model: &Model, i: usize, j: usize, _time: f32, modifiers: [f32; 8]) {
	let [_whole, _half, quarter, _eighth, _sixteenth, _bass, _mid, _treble] = modifiers;
	let unit = model.scaling as f32;
	let (x, y) = scale_coordinates(model, i, j);
	let lines = model.reveal(Unlock::Lines);
	let lit = glow(model, pulse(&modifiers), false);
	match model.hint_at(i, j) {
		// The patch.
		Some(1) => dot(draw, model, x, y, paint(model, 0.9 * glow(model, pulse(&modifiers), true), 0.0)),
		// The gap: bare points, pulsing on the beat.
		Some(0) => dot(draw, model, x, y, paint(model, (0.55 + 0.45 * (1.0 - quarter)) * lit, 0.0)),
		_ if j % 2 == 0 => {
			// A stripe: the point grows its line to the next point.
			let gray = (0.7 + 0.2 * lines) * lit;
			mark(draw, model, x, y, 0.0, 0.0, unit, gray, 0.0);
		}
		_ => {
			// The rows between: there until the stripes are.
			dot(draw, model, x, y, paint(model, 0.7 * lit * (1.0 - lines), 0.0));
		}
	}
}

/// Lines and dots in a checkerboard; the lines lie along the grid, one cell long, still.
/// Drawn like that, no two lines can ever join up — except around the one box whose edge
/// cells all draw lines, head to tail: the only bounded area. Tap it. Meanwhile a large
/// region glows in inverse, and hidden in it is a second box whose cells never reach black
/// or white.
const BOX: Area = area(12, 3, 18, 9);
const INVERSE: Area = area(0, 10, 10, 20);
const HIDDEN: Area = area(3, 13, 7, 17);

/// The direction a cell on the edge of `area` draws in to go round it clockwise.
fn around(area: Area, i: usize, j: usize) -> f32 {
	if j + 1 == area.j1 && i + 1 < area.i1 {
		0.0
	} else if i + 1 == area.i1 && j > area.j0 {
		-PI / 2.0
	} else if j == area.j0 && i > area.i0 {
		PI
	} else {
		PI / 2.0
	}
}

/// The checkerboard field shared by the last two phases of the act.
fn checker(draw: &Draw, model: &Model, i: usize, j: usize, pulse: f32, length: f32) {
	let (x, y) = scale_coordinates(model, i, j);
	let noise = model.noisemap.get_value(i, j) as f32;
	let lit = if HIDDEN.contains(i, j) {
		// Bounded to the greys.
		1.0 + ((0.35 + 0.3 * pulse) - 1.0) * model.reveal(Unlock::Glow)
	} else {
		glow(model, pulse, INVERSE.contains(i, j))
	};
	if BOX.on_perimeter(i, j) {
		mark(draw, model, x, y, around(BOX, i, j), 0.0, length, 0.95 * lit, 0.0);
	} else if (i + j) % 2 == 0 {
		// Along or across, about half and half, scattered by the noise.
		let angle = if (noise * 10.0).floor() as i32 % 2 == 0 { 0.0 } else { PI / 2.0 };
		mark(draw, model, x, y, angle, 0.0, length, 0.7 * lit, 0.0);
	} else {
		dot(draw, model, x, y, paint(model, 0.5 * lit, 0.0));
	}
}

fn boxes(draw: &Draw, model: &Model, i: usize, j: usize, _time: f32, modifiers: [f32; 8]) {
	let [_whole, _half, _quarter, _eighth, _sixteenth, _bass, _mid, _treble] = modifiers;
	let unit = model.scaling as f32;
	checker(draw, model, i, j, pulse(&modifiers), unit);
}

/// The same field with a cross through the middle. Tap the centre: lengths start to move.
const CENTRE: Area = area(8, 8, 12, 12);

fn cross(draw: &Draw, model: &Model, i: usize, j: usize, _time: f32, modifiers: [f32; 8]) {
	let [_whole, _half, _quarter, _eighth, sixteenth, bass, _mid, _treble] = modifiers;
	let unit = model.scaling as f32;
	let (x, y) = scale_coordinates(model, i, j);
	let length = unit * (0.3 + 1.4 * bass + 0.6 * (1.0 - sixteenth));
	if i == 10 || j == 10 {
		let angle = if j == 10 { 0.0 } else { PI / 2.0 };
		mark(draw, model, x, y, angle, 0.0, length, 0.95 * glow(model, pulse(&modifiers), false), 0.0);
		return;
	}
	checker(draw, model, i, j, pulse(&modifiers), length);
}

// ---- sasha grips ----

/// The growly part. Around every dot a point orbits, and a line runs from the dot to it.
/// A second, invisible point orbits further out the other way: when the bass hits, the
/// tip of the line is pulled toward it, and eases back as the bass fades.
fn orbits(draw: &Draw, model: &Model, i: usize, j: usize, time: f32, modifiers: [f32; 8]) {
	let [whole, _half, _quarter, _eighth, _sixteenth, bass, _mid, _treble] = modifiers;
	let unit = model.scaling as f32;
	let (x, y) = scale_coordinates(model, i, j);
	let noise = model.noisemap.get_value(i, j) as f32;
	let lit = glow(model, pulse(&modifiers), false);

	let near = (0.8 + 0.4 * noise) * time + noise * TAU;
	let far = -(1.3 + 0.5 * noise) * time + noise * 7.0;
	let (x1, y1) = (x + 0.45 * unit * near.cos(), y + 0.45 * unit * near.sin());
	let (x2, y2) = (x + 0.9 * unit * far.cos(), y + 0.9 * unit * far.sin());

	// The pull: nothing until the bass is really there, all the way at full.
	let pull = ((bass - 0.25) / 0.5).clamp(0.0, 1.0);
	let pull = pull * pull * (3.0 - 2.0 * pull);
	let (ex, ey) = (x1 + (x2 - x1) * pull, y1 + (y2 - y1) * pull);

	let mut gray = 0.6 + 0.3 * pull;
	if model.hint_at(i, j) == Some(0) {
		gray = 1.0 - 0.6 * whole;
	}
	dot(draw, model, x, y, paint(model, 0.9 * lit, 0.0));
	dot(draw, model, x1, y1, paint(model, 0.5 * lit, 0.0));
	mark(
		draw,
		model,
		x,
		y,
		(ey - y).atan2(ex - x),
		0.0,
		((ex - x).powi(2) + (ey - y).powi(2)).sqrt(),
		gray * lit,
		pull,
	);
}

// ---- the placeholder phases ----

/// One rendering for every phase still to be designed, varied by `k` (its index in the
/// album) so no two look quite the same: the field leans a different way, turns at a
/// different rate.
fn render(draw: &Draw, model: &Model, i: usize, j: usize, time: f32, modifiers: [f32; 8], k: usize) {
	let [whole, _half, _quarter, _eighth, sixteenth, bass, mid, treble] = modifiers;
	let noise = model.noisemap.get_value(i, j) as f32;
	let unit = model.scaling as f32;
	let (x, y) = scale_coordinates(model, i, j);
	let k = k as f32;

	let hot = SQUARES.is_current(model, Subdivision::Sixteenth, i, j);
	let mut length = unit * (0.3 + 0.9 * mid + if hot { 0.8 * (1.0 - sixteenth) } else { 0.0 });
	let mut gray = 0.15 + 0.6 * noise.abs() * (0.4 + treble) + if hot { 0.25 } else { 0.0 };
	let angle = noise * TAU + k * 0.37;
	let turn = time * (0.15 + 0.05 * (k % 5.0)) * if (i + j) % 2 == 0 { 1.0 } else { -1.0 };
	let color = (noise.abs() + 0.5 * bass).min(1.0);

	if model.hint_at(i, j) == Some(0) {
		// The door breathes with the bar.
		gray = 1.0 - 0.7 * whole;
		length = unit * (0.6 + 0.4 * bass);
	}
	if let Some(transition) = model.transition() {
		length *= 1.0 + 3.0 * transition;
	}
	mark(draw, model, x, y, angle, turn, length, gray * glow(model, pulse(&modifiers), false), color);
}

macro_rules! phase {
	($id:literal, $track:expr, $bars:expr, $next:literal, $k:expr) => {
		&Scene {
			id: $id,
			track: &$track,
			bars: $bars,
			render: &|draw, model, i, j, time, modifiers| {
				render(draw, model, i, j, time, modifiers, $k)
			},
			hotspots: &[Hotspot {
				cells: DOOR,
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Go($next),
				tag: 0,
			}],
		}
	};
}

pub const SCENES: &[&Scene] = &[
	// sine — 135 bpm, 84 bars: four 16-bar blocks, then the bass goes, then everything but
	// the tone. The first act.
	&Scene {
		id: "sine/0",
		track: &SINE,
		bars: 0..16,
		render: &|draw, model, i, j, time, modifiers| met_ends(draw, model, i, j, time, modifiers),
		hotspots: &[Hotspot {
			cells: E_IN_MET,
			needs: NONE,
			on: Gesture::Tap,
			then: Action::Go("sine/1"),
			tag: 0,
		}],
	},
	&Scene {
		id: "sine/1",
		track: &SINE,
		bars: 16..32,
		render: &|draw, model, i, j, time, modifiers| square(draw, model, i, j, time, modifiers),
		hotspots: &[
			Hotspot {
				cells: SQUARE,
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Unlock(Unlock::Glow),
				tag: 1,
			},
			Hotspot {
				cells: SQUARE_2,
				needs: Unlocks::of(&[Unlock::Glow]),
				on: Gesture::Tap,
				then: Action::Go("sine/2"),
				tag: 0,
			},
		],
	},
	&Scene {
		id: "sine/2",
		track: &SINE,
		bars: 32..48,
		render: &|draw, model, i, j, time, modifiers| stripes(draw, model, i, j, time, modifiers),
		hotspots: &[
			Hotspot {
				cells: PATCH,
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Unlock(Unlock::Lines),
				tag: 1,
			},
			Hotspot {
				cells: GAP,
				needs: Unlocks::of(&[Unlock::Lines]),
				on: Gesture::Outline,
				then: Action::Go("sine/3"),
				tag: 0,
			},
		],
	},
	&Scene {
		id: "sine/3",
		track: &SINE,
		bars: 48..64,
		render: &|draw, model, i, j, time, modifiers| boxes(draw, model, i, j, time, modifiers),
		hotspots: &[Hotspot {
			cells: BOX,
			needs: NONE,
			on: Gesture::Tap,
			then: Action::Go("sine/4"),
			tag: 0,
		}],
	},
	&Scene {
		id: "sine/4",
		track: &SINE,
		bars: 64..72,
		render: &|draw, model, i, j, time, modifiers| cross(draw, model, i, j, time, modifiers),
		hotspots: &[
			Hotspot {
				cells: CENTRE,
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Unlock(Unlock::Length),
				tag: 1,
			},
			Hotspot {
				cells: CENTRE,
				needs: Unlocks::of(&[Unlock::Length]),
				on: Gesture::Tap,
				then: Action::Go("sine/5"),
				tag: 0,
			},
		],
	},
	phase!("sine/5", SINE, 72..84, "sasha-grips/0", 5),
	// sasha grips — 147 bpm, 88 bars: full from the first bar, a 4-bar bass drop at 61.
	phase!("sasha-grips/0", SASHA_GRIPS, 0..8, "sasha-grips/1", 6),
	phase!("sasha-grips/1", SASHA_GRIPS, 8..32, "sasha-grips/2", 7),
	&Scene {
		id: "sasha-grips/2",
		track: &SASHA_GRIPS,
		bars: 32..60,
		render: &|draw, model, i, j, time, modifiers| orbits(draw, model, i, j, time, modifiers),
		hotspots: &[Hotspot {
			cells: DOOR,
			needs: NONE,
			on: Gesture::Tap,
			then: Action::Go("sasha-grips/3"),
			tag: 0,
		}],
	},
	phase!("sasha-grips/3", SASHA_GRIPS, 60..64, "sasha-grips/4", 9),
	phase!("sasha-grips/4", SASHA_GRIPS, 64..88, "pisser/0", 10),
	// pisser — 120 bpm, 104 bars: near silence, a build without bass, four 16-bar cycles
	// each dipping in its last 4 bars, an outro.
	phase!("pisser/0", PISSER, 0..8, "pisser/1", 11),
	phase!("pisser/1", PISSER, 8..24, "pisser/2", 12),
	phase!("pisser/2", PISSER, 24..40, "pisser/3", 13),
	phase!("pisser/3", PISSER, 40..56, "pisser/4", 14),
	phase!("pisser/4", PISSER, 56..72, "pisser/5", 15),
	phase!("pisser/5", PISSER, 72..88, "pisser/6", 16),
	phase!("pisser/6", PISSER, 88..104, "for-against/0", 17),
	// for.against — 128 bpm, 148 bars + tail: intro without bass, the groove, a filtered
	// break at 37, the build from 61, the drop at 77 (the project's 'drop' marker), a
	// breakdown at 109, the long way out.
	phase!("for-against/0", FOR_AGAINST, 0..12, "for-against/1", 18),
	phase!("for-against/1", FOR_AGAINST, 12..36, "for-against/2", 19),
	phase!("for-against/2", FOR_AGAINST, 36..44, "for-against/3", 20),
	phase!("for-against/3", FOR_AGAINST, 44..60, "for-against/4", 21),
	phase!("for-against/4", FOR_AGAINST, 60..76, "for-against/5", 22),
	phase!("for-against/5", FOR_AGAINST, 76..108, "for-against/6", 23),
	phase!("for-against/6", FOR_AGAINST, 108..116, "for-against/7", 24),
	phase!("for-against/7", FOR_AGAINST, 116..148, "sine/0", 25),
];
