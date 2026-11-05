//! The album: the real tracks, cut into their phases. Each phase is a scene looping its
//! bars; a tap in the centre leads to the next phase, the last phase of a track to the
//! first of the next track, and the end of the album back to its start. Linear for now —
//! the doors, side paths and unlock puzzles are the owner's to design per phase.
//!
//! The cuts come from `tools/phases.py` (see the bar maps in `memory/notes.md`); the
//! visuals here are one placeholder rendering, varied a little per phase, so that every
//! phase is at least visibly different and reacts to its own music.

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

/// The way on, in every phase for now.
const DOOR: Area = area(8, 8, 12, 12);

/// Sixteenths walk across the screen.
const SQUARES: Regions = Regions::Grid { cols: 4, rows: 4 };

/// The gap in the stripes of the first phase: where the first line gets drawn.
const GAP: Area = area(6, 10, 14, 11);

/// The first phase: stripes. Every other row is a line of points joined by a stripe; one
/// stripe has a gap where the points are bare and glow. Draw through them and the points
/// become lines — horizontal, like the stripes, which dissolve into them. Only then is
/// there a door.
fn stripes(draw: &Draw, model: &Model, i: usize, j: usize, time: f32, modifiers: [f32; 8]) {
	let [whole, _half, quarter, _eighth, _sixteenth, bass, _mid, treble] = modifiers;
	if j % 2 == 1 {
		return;
	}
	let unit = model.scaling as f32;
	let (x, y) = scale_coordinates(model, i, j);
	let lines = model.reveal(Unlock::Lines);
	let hint = model.hint_at(i, j);

	if hint == Some(1) {
		// The gap: bare points, pulsing on the beat.
		mark(draw, model, x, y, 0.0, 0.0, unit, 0.55 + 0.45 * (1.0 - quarter), 0.0);
		return;
	}
	// The stripe on to the next point, fading as the lines come to take its place.
	if i + 1 < CANVAS_WIDTH && lines < 1.0 {
		stroke(draw, model, x, y, 0.0, unit, paint(model, 0.3 * (1.0 - lines), 0.0));
	}
	let mut gray = 0.5 + 0.4 * bass;
	let mut length = unit;
	if hint == Some(0) {
		// The door breathes with the bar.
		gray = 1.0 - 0.7 * whole;
	}
	if let Some(transition) = model.transition() {
		length *= 1.0 + 3.0 * transition;
	}
	mark(draw, model, x, y, 0.0, time * 0.3, length, gray, treble);
}

/// One rendering for every phase, varied by `k` (its index in the album) so no two phases
/// look the same: the field leans a different way, turns at a different rate.
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
	mark(draw, model, x, y, angle, turn, length, gray, color);
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
	// sine — 135 bpm, 84 bars: four 16-bar blocks, then the bass goes, then everything but the tone.
	&Scene {
		id: "sine/0",
		track: &SINE,
		bars: 0..16,
		render: &|draw, model, i, j, time, modifiers| stripes(draw, model, i, j, time, modifiers),
		hotspots: &[
			Hotspot {
				cells: GAP,
				needs: NONE,
				on: Gesture::Outline,
				then: Action::Unlock(Unlock::Lines),
				tag: 1,
			},
			Hotspot {
				cells: DOOR,
				needs: Unlocks::of(&[Unlock::Lines]),
				on: Gesture::Tap,
				then: Action::Go("sine/1"),
				tag: 0,
			},
		],
	},
	phase!("sine/1", SINE, 16..32, "sine/2", 1),
	phase!("sine/2", SINE, 32..48, "sine/3", 2),
	phase!("sine/3", SINE, 48..64, "sine/4", 3),
	phase!("sine/4", SINE, 64..72, "sine/5", 4),
	phase!("sine/5", SINE, 72..84, "sasha-grips/0", 5),
	// sasha grips — 147 bpm, 88 bars: full from the first bar, a 4-bar bass drop at 61.
	phase!("sasha-grips/0", SASHA_GRIPS, 0..8, "sasha-grips/1", 6),
	phase!("sasha-grips/1", SASHA_GRIPS, 8..32, "sasha-grips/2", 7),
	phase!("sasha-grips/2", SASHA_GRIPS, 32..60, "sasha-grips/3", 8),
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
