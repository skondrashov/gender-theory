//! for / against.
//!
//! The demo graph (first seven scenes): a loop through the unlocks with side doors, then
//! the original six scenes from before there was a graph, kept as `old/*`.
//!
//! Unlocks never move you: you watch them land on the scene you're on, and the door that
//! needs them appears there. A transition (`GoVia`) plays a passage of the track you're on
//! while the scene intensifies, then switches.
//!
//! ```text
//! points ──tap centre──> points_2 ──tap black square──> points
//!   │                       │ [lines] ──> spin        │ [circles] ──> color_gate
//!   └─tap corner: +lines, then [lines] ──> lines: +length, then [length] ──> length
//!   length: +rotation, then [rotation] ──via fa/1 0..2──> spin: draw the box: +circles,
//!   then tap the box [circles] ──via fa/2 10..14──> color_gate: +color, then [color] ──> points
//! ```

use {
	crate::scenes::*,
	nannou::{color::luma::Luma, noise::NoiseFn},
};

pub static BU_2: Track = Track {
	id: "bu/2",
	path: "/bu/2.mp3",
	bpm: 73.2802, // as the old hand count had it
};
pub static FA_1: Track = Track {
	id: "fa/1",
	path: "/fa/1.mp3",
	bpm: 62.5, // as the old hand count had it
};
pub static FA_2: Track = Track {
	id: "fa/2",
	path: "/fa/2.mp3",
	bpm: 109.375, // as the old hand count had it
};

/// The centre 5×5 cells.
const CENTRE: Area = area(8, 8, 13, 13);
/// Top-right corner.
const CORNER: Area = area(15, 15, 19, 19);
/// The box to draw in `spin`: 7×5 cells.
const BOX: Area = area(6, 7, 13, 12);
/// The one centre cell: the old ±5 px click box.
const DOT: Area = area(10, 10, 11, 11);

/// Where cell (i, j) points when the `spin` scene lines up: away from `BOX`. Outside the
/// box, straight away from the nearest side (so the field is columns above and below it,
/// rows beside it). On the box's edge, out through that side (corners: up or down).
/// Inside, toward the nearest side.
fn box_direction(i: usize, j: usize) -> f32 {
	const UP: f32 = PI / 2.0;
	const LEFT: f32 = PI;
	const DOWN: f32 = 3.0 * PI / 2.0;
	const RIGHT: f32 = 0.0;
	let Area { i0, j0, i1, j1 } = BOX;
	if j >= j1 {
		UP
	} else if j < j0 {
		DOWN
	} else if i < i0 {
		LEFT
	} else if i >= i1 {
		RIGHT
	} else if j + 1 == j1 || j + 2 == j1 {
		UP
	} else if j == j0 || j == j0 + 1 {
		DOWN
	} else if i == i0 || i == i0 + 1 {
		LEFT
	} else if i + 1 == i1 || i + 2 == i1 {
		RIGHT
	} else {
		UP
	}
}

pub const SCENES: &[&Scene] = &[
	// ---- the demo graph ----
	&Scene {
		id: "points",
		track: &BU_2,
		bars: 0..8,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, time, modifiers| {
			let [whole, _half, quarter, _eighth, _sixteenth, bass, _mid, treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let unit = model.scaling as f32;
			let (x, y) = scale_coordinates(model, i, j);
			let gray = match model.hint_at(i, j) {
				// The way on breathes on the beat.
				Some(0) => 0.5 + 0.5 * (1.0 - quarter),
				// The way sideways flashes once a bar.
				Some(_) => 0.2 + 0.8 * (1.0 - whole).powi(3),
				None => 0.15 + 0.6 * noise.abs(),
			};
			mark(
				draw,
				model,
				x,
				y,
				noise * TAU,
				time * 0.2,
				unit * (0.4 + bass),
				gray,
				treble,
			);
		},
		hotspots: &[
			Hotspot {
				cells: CENTRE,
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Go("points_2"),
				tag: 0,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: CORNER,
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Unlock(Unlock::Lines),
				tag: 1,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: CORNER,
				needs: Unlocks::of(&[Unlock::Lines]),
				on: Gesture::Tap,
				then: Action::Go("lines"),
				tag: 1,
				after: &[],
				stage: None,
				inside: None,
			},
		],
	},
	&Scene {
		id: "points_2",
		track: &BU_2,
		bars: 0..8,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, time, modifiers| {
			let [_whole, half, _quarter, _eighth, _sixteenth, _bass, mid, _treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let unit = model.scaling as f32;
			let (x, y) = scale_coordinates(model, i, j);
			match model.hint_at(i, j) {
				// The black square: the way back.
				Some(0) => {}
				// Spins, once you can see that.
				Some(1) => mark(draw, model, x, y, 0.0, time, unit, 1.0, 1.0),
				// Rings, once you can see those.
				Some(_) => ring(
					draw,
					model,
					x,
					y,
					unit * 0.4 * (0.5 + mid) * model.reveal(Unlock::Rings),
					1.5,
					paint(model, 0.8, 1.0),
				),
				None => mark(
					draw,
					model,
					x,
					y,
					noise * 3.0,
					0.0,
					unit * 0.6,
					noise.abs() * (0.3 + 0.7 * half),
					mid,
				),
			}
		},
		hotspots: &[
			Hotspot {
				cells: CENTRE,
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Go("points"),
				tag: 0,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: area(1, 1, 5, 5),
				needs: Unlocks::of(&[Unlock::Lines]),
				on: Gesture::Tap,
				then: Action::Go("spin"),
				tag: 1,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: area(15, 1, 19, 5),
				needs: Unlocks::of(&[Unlock::Rings]),
				on: Gesture::Tap,
				then: Action::Go("color_gate"),
				tag: 2,
				after: &[],
				stage: None,
				inside: None,
			},
		],
	},
	&Scene {
		id: "lines",
		track: &FA_1,
		bars: 0..7,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, _time, modifiers| {
			let [_whole, _half, _quarter, eighth, _sixteenth, _bass, mid, _treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let unit = model.scaling as f32;
			let (x, y) = scale_coordinates(model, i, j);
			let gray = if model.hint_at(i, j).is_some() {
				1.0
			} else {
				0.25 + 0.5 * (1.0 - eighth) * noise.abs()
			};
			mark(draw, model, x, y, noise * TAU, 0.0, unit * (0.6 + 0.6 * mid), gray, noise.abs());
		},
		hotspots: &[
			Hotspot {
				cells: area(8, 2, 13, 4),
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Unlock(Unlock::Length),
				tag: 0,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: area(8, 2, 13, 4),
				needs: Unlocks::of(&[Unlock::Length]),
				on: Gesture::Tap,
				then: Action::Go("length"),
				tag: 0,
				after: &[],
				stage: None,
				inside: None,
			},
		],
	},
	&Scene {
		id: "length",
		track: &FA_1,
		bars: 7..14,
		loop_from: 7,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, _time, modifiers| {
			const SQUARES: Regions = Regions::Grid { cols: 4, rows: 4 };
			let [_whole, _half, _quarter, _eighth, sixteenth, bass, _mid, _treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let unit = model.scaling as f32;
			let (x, y) = scale_coordinates(model, i, j);
			// One square per 16th stretches out and snaps back.
			let hot = SQUARES.is_current(model, Subdivision::Sixteenth, i, j);
			let mut length = if hot {
				unit * (0.3 + 2.0 * (1.0 - sixteenth))
			} else {
				unit * 0.5
			};
			if let Some(transition) = model.transition() {
				// On the way out everything stretches.
				length *= 1.0 + 3.0 * transition;
			}
			let gray = match model.hint_at(i, j) {
				Some(0) => 1.0,
				// The way back: a black square.
				Some(1) => 0.0,
				Some(_) => 1.0,
				None if hot => 0.9,
				None => 0.3 + 0.3 * bass,
			};
			mark(draw, model, x, y, noise * TAU, 0.0, length, gray, if hot { 1.0 } else { 0.2 });
		},
		hotspots: &[
			Hotspot {
				cells: area(2, 14, 6, 18),
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Unlock(Unlock::Angle),
				tag: 0,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: area(2, 14, 6, 18),
				needs: Unlocks::of(&[Unlock::Angle]),
				on: Gesture::Tap,
				then: Action::GoVia("spin", 0..2),
				tag: 2,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: area(9, 9, 11, 11),
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Go("lines"),
				tag: 1,
				after: &[],
				stage: None,
				inside: None,
			},
		],
	},
	&Scene {
		id: "spin",
		track: &FA_2,
		bars: 2..10,
		loop_from: 2,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, _time, modifiers| {
			let [whole, _half, _quarter, _eighth, _sixteenth, bass, _mid, _treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let unit = model.scaling as f32;
			let (x, y) = scale_coordinates(model, i, j);

			// Every line turns at its own whole number of turns per 4 bars, so once every 4
			// bars, for an instant, they all come round together: one grid interval long,
			// pointing away from the box. Except the cells on the box's edge, which come
			// round pointing *in*, onto the line of the cell inside them: doubled up. So
			// the lines that should run outward from the edge are the ones missing, and
			// the box shows as the band of nothing around it. Draw its edge to take it.
			let cycle = ((model.ticks(Subdivision::Whole) % 4) as f32 + whole) / 4.0;
			// Ease a little around the alignment so it reads, without stopping.
			let s = cycle - 0.05 * (TAU * cycle).sin();
			let turns = 1.0 + (noise.abs() * 3.0).floor();
			let on_box = BOX.on_perimeter(i, j);
			let aligned = box_direction(i, j) + if on_box { PI } else { 0.0 };
			// Without rotation you see one still frame from mid-cycle: no box in it.
			const STILL: f32 = 0.37;
			let angle = aligned + turns * TAU * STILL;
			let turn = turns * TAU * (s - STILL);
			let length = unit * (1.0 + 0.6 * (PI * s).sin() * noise.abs());

			let mut gray = 0.5 + 0.4 * bass;
			if model.hint_at(i, j) == Some(1) {
				// The way back.
				gray = 1.0 - whole;
			}
			let transition = model.transition().unwrap_or(0.0);
			if model.hint_at(i, j) == Some(2) || transition > 0.0 {
				// The box, once you can see circles: the way on. On the way out the
				// circles spread to every cell and open up.
				let spread = if model.hint_at(i, j) == Some(2) { 1.0 } else { transition };
				ring(
					draw,
					model,
					x,
					y,
					unit * (0.35 + 2.0 * transition * noise.abs())
						* (0.5 + bass) * model.reveal(Unlock::Rings),
					1.5,
					paint(model, gray * spread, 1.0),
				);
			}
			mark(draw, model, x, y, angle, turn, length, gray, s);
		},
		hotspots: &[
			Hotspot {
				cells: BOX,
				needs: Unlocks::of(&[Unlock::Angle]),
				on: Gesture::Outline,
				then: Action::Unlock(Unlock::Rings),
				tag: 0,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: BOX,
				needs: Unlocks::of(&[Unlock::Rings]),
				on: Gesture::Tap,
				then: Action::GoVia("color_gate", 10..14),
				tag: 2,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: area(0, 0, 2, 2),
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Go("length"),
				tag: 1,
				after: &[],
				stage: None,
				inside: None,
			},
		],
	},
	&Scene {
		id: "color_gate",
		track: &BU_2,
		bars: 0..8,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, time, modifiers| {
			let [_whole, half, _quarter, _eighth, _sixteenth, bass, _mid, _treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let unit = model.scaling as f32;
			let (x, y) = scale_coordinates(model, i, j);
			match model.hint_at(i, j) {
				// Rings pumping with the bass: only there once you can see circles. They
				// take the colour first, then lead home.
				Some(0) | Some(2) => ring(
					draw,
					model,
					x,
					y,
					unit * (0.3 + 0.5 * bass) * model.reveal(Unlock::Rings),
					2.0,
					paint(model, 0.6 + 0.4 * bass, 1.0),
				),
				// The top row: back to the start.
				Some(_) => mark(draw, model, x, y, 0.0, time, unit, 0.9, 0.0),
				None => mark(
					draw,
					model,
					x,
					y,
					noise * TAU,
					0.0,
					unit * 0.5,
					0.2 + 0.3 * half * noise.abs(),
					half,
				),
			}
		},
		hotspots: &[
			Hotspot {
				cells: CENTRE,
				needs: Unlocks::of(&[Unlock::Rings]),
				on: Gesture::Tap,
				then: Action::Unlock(Unlock::Color),
				tag: 0,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: CENTRE,
				needs: Unlocks::of(&[Unlock::Color]),
				on: Gesture::Tap,
				then: Action::Go("points"),
				tag: 2,
				after: &[],
				stage: None,
				inside: None,
			},
			Hotspot {
				cells: area(0, 18, 20, 20),
				needs: NONE,
				on: Gesture::Tap,
				then: Action::Go("points"),
				tag: 1,
				after: &[],
				stage: None,
				inside: None,
			},
		],
	},
	// ---- the scenes from before there was a graph, as they were ----
	&Scene {
		id: "old/1",
		track: &BU_2,
		bars: 0..8,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, _time, _modifiers| {
			let noise = model.noisemap.get_value(i, j) as f32;
			let (i, j) = scale_coordinates(model, i, j);
			let gray = noise;
			// draw.text(&format!("{noise:.2}")).x_y(i, j).gray(1.0);

			draw.line()
				.gray(gray)
				.points(Point2::new(i, j), Point2::new(i + 1.0, j + 1.0));
		},
		hotspots: &[Hotspot {
			cells: CENTRE,
			needs: NONE,
			on: Gesture::Tap,
			then: Action::Go("old/2"),
			tag: 0,
			after: &[],
			stage: None,
			inside: None,
		}],
	},
	&Scene {
		id: "old/2",
		track: &BU_2,
		bars: 0..8,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, time, modifiers| {
			let [whole, half, quarter, eighth, _sixteenth, _bass, mid, treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let noise2 = model
				.noise_matrix
				.get([i as f64, j as f64, mid as f64 / 10.0]) as f32;
			let hint = model.hint_at(i, j).is_some();
			let (i, j) = scale_coordinates(model, i, j);
			let gray = if hint {
				1.0
			} else {
				(half
					+ 0.2 * (quarter - 1.0).abs()
					+ 0.4 * mid + 0.2 * whole
					+ 0.4 * noise * treble)
					- 0.5
			};
			let (i0, i1) = (
				i + 30.0 * (time * noise).cos(),
				i + 25.0 * (noise * mid).cos() + 20.0 * (time + noise2 * eighth / 10.0).sin(),
			);
			let (j0, j1) = (
				j + 30.0 * (noise * 60.0 + time + time * noise2).sin(),
				j - 20.0 * (time + noise * 5.0 * mid).sin()
					- 20.0 * (time + noise2 * half / 20.0).cos(),
			);

			draw.line()
				.gray(gray)
				.points(Point2::new(i0, j0), Point2::new(i1, j1));
		},
		hotspots: &[Hotspot {
			cells: CENTRE,
			needs: NONE,
			on: Gesture::Tap,
			then: Action::Go("old/3"),
			tag: 0,
			after: &[],
			stage: None,
			inside: None,
		}],
	},
	&Scene {
		id: "old/3",
		track: &FA_1,
		bars: 0..14,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, time, modifiers| {
			let [whole, half, quarter, _eighth, _sixteenth, _bass, mid, _treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			// let noise2 = model
			// 	.noise_matrix
			// 	.get([i as f64, j as f64, (mid * time) as f64 / 10.0]) as f32;
			let hint = model.hint_at(i, j).is_some();
			let (i, j) = scale_coordinates(model, i, j);
			let gray = if hint {
				1.0
			} else {
				half + 0.2 * (quarter - 1.0).abs() + 0.4 * mid + 0.2 * whole + 0.4 * noise
			};
			let (i0, i1) = (i, i + 25.0 * (time * 1.0001 * noise).cos());
			let (j0, j1) = (j, j - 20.0 * (time + noise * 5.0).sin());

			draw.line()
				.gray(gray)
				.points(Point2::new(i0, j0), Point2::new(i1, j1));
		},
		hotspots: &[Hotspot {
			cells: CENTRE,
			needs: NONE,
			on: Gesture::Tap,
			then: Action::Go("old/4"),
			tag: 0,
			after: &[],
			stage: None,
			inside: None,
		}],
	},
	&Scene {
		id: "old/4",
		track: &FA_1,
		bars: 0..14,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, time, modifiers| {
			let [whole, half, quarter, _eighth, _sixteenth, bass, _mid, _treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let noise2 = model
				.noise_matrix
				.get([i as f64, j as f64, time as f64 / 10.0]) as f32;
			let hint = model.hint_at(i, j).is_some();

			let (i, j) = scale_coordinates(model, i, j);

			let mut gray = half + 0.2 * (quarter - 1.0).abs() + 0.4 * bass;
			let (i0, i1) = (i, i + 15.0 * (time * noise).cos());
			let (j0, j1) = (j, j - 10.0 * (time + noise * 10.0).sin());

			if hint {
				gray = 1.0
			}

			draw.ellipse()
				.no_fill()
				.stroke_weight(2.0)
				.stroke_color(Luma::new(gray))
				.x_y(i0, j0)
				.radius(40.0 * bass * noise2 + 15.0 * whole * noise);

			draw.line()
				.gray(gray)
				.points(Point2::new(i0, j0), Point2::new(i1, j1));
		},
		hotspots: &[Hotspot {
			cells: DOT,
			needs: NONE,
			on: Gesture::Tap,
			then: Action::Go("old/5"),
			tag: 0,
			after: &[],
			stage: None,
			inside: None,
		}],
	},
	&Scene {
		id: "old/5",
		track: &FA_2,
		bars: 0..14,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, time, modifiers| {
			let [_whole, half, quarter, _eighth, _sixteenth, bass, _mid, _treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let hint = model.hint_at(i, j).is_some();

			let (i, j) = scale_coordinates(model, i, j);

			let mut gray = half + 0.2 * (quarter - 1.0).abs() + 0.4 * bass;
			let (i0, i1) = (i, i + 25.0 * (time * noise).cos());
			let (j0, j1) = (j, j + 20.0);

			if hint {
				gray = 1.0
			}

			draw.line()
				.gray(gray)
				.points(Point2::new(i0, j0), Point2::new(i1, j1));
		},
		hotspots: &[Hotspot {
			cells: DOT,
			needs: NONE,
			on: Gesture::Tap,
			then: Action::Go("old/6"),
			tag: 0,
			after: &[],
			stage: None,
			inside: None,
		}],
	},
	&Scene {
		id: "old/6",
		track: &FA_2,
		bars: 0..14,
		loop_from: 0,
		stages: &[],
		delays: &[],
		render: &|draw, model, i, j, time, modifiers| {
			let [_whole, half, quarter, eighth, sixteenth, bass, mid, treble] = modifiers;
			let noise = model.noisemap.get_value(i, j) as f32;
			let noise2 = model.noise_matrix.get([i as f64, j as f64, time as f64]) as f32;

			let (i, j) = scale_coordinates(model, i, j);

			let mut gray = half * noise * noise2 * 3.0
				+ 0.2 * (quarter - 1.0).abs()
				+ 0.7 * noise2 * noise2 * noise2
				+ 0.4 * bass;
			let (i0, i1) = (
				i + 10.0 * noise2 * mid * treble + 5.0 * time.cos(),
				(i + 20.0 * noise) + eighth,
			);
			let (j0, j1) = (
				j - 12.0 * quarter.sin() - noise2 * 10.0 + 5.0 * time.sin(),
				(j + 3.0 * noise.cos() * sixteenth * eighth * eighth.sin()),
			);

			// Whichever line's start wanders within 5 px of the centre lights up.
			if i0.abs() < 5.0 && j0.abs() < 5.0 {
				gray = 1.0
			}

			draw.line()
				.gray(gray)
				.points(Point2::new(i0, j0), Point2::new(i1, j1));
		},
		hotspots: &[Hotspot {
			cells: DOT,
			needs: NONE,
			on: Gesture::Tap,
			then: Action::Go("old/1"),
			tag: 0,
			after: &[],
			stage: None,
			inside: None,
		}],
	},
];
