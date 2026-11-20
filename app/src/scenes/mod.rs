use {
	crate::model::{Model, CANVAS_HEIGHT, CANVAS_WIDTH},
	nannou::{prelude::*, Draw},
	std::ops::Range,
};

pub mod data;
pub mod font;
pub mod fields;
// pub mod being_useless;
pub mod for_against;
// pub mod porcelain_piss;
pub mod terra_firmament;

/// One mp3 and its bar grid: bar 0 starts at t = 0 and bars are `240 / bpm` seconds. Every
/// scene on the track counts bars on this grid. `tools/phases.py` finds the tempo and the
/// sections of a track; Ableton bounces start on a bar line, so t = 0 is the downbeat.
pub struct Track {
	pub id: &'static str,
	pub path: &'static str,
	pub bpm: f64,
}

impl Track {
	/// Seconds per bar.
	pub fn bar(&self) -> f64 {
		240.0 / self.bpm
	}
}

/// A stretch of a track, a way of seeing it, and the things you can do in it. The stretch
/// plays once from its first bar, then its tail (from `loop_from`) repeats until the player
/// leaves. Inside it, `stages` are the visual states the player taps through without the
/// music changing (hotspots with `Action::Stage`); layers and hotspots can belong to one.
pub struct Scene {
	pub id: &'static str,
	pub track: &'static Track,
	/// The stretch, in bars of `track` (end exclusive).
	pub bars: Range<usize>,
	/// The bar the tail loop starts on (within `bars`); `bars.start` loops the whole stretch.
	pub loop_from: usize,
	/// Names of the stages, in order. Empty means one unnamed stage.
	pub stages: &'static [&'static str],
	/// Called once per grid cell per frame: (draw, model, i, j, time, modifiers).
	pub render: &'static dyn Fn(&Draw, &Model, usize, usize, f32, [f32; 8]),
	pub hotspots: &'static [Hotspot],
}

/// Something to do in a scene. Invisible and inert until the player has all of `needs`.
/// The scene's render draws the hint itself, by asking `model.hint_at(i, j)` for the `tag`.
pub struct Hotspot {
	/// The rectangle round its cells: what the outline gesture traces, what hints go round.
	pub cells: Area,
	/// Narrows `cells` to a shape (a painted map, a checker, …); `None`: the whole rectangle.
	pub inside: Option<&'static (dyn Fn(usize, usize) -> bool + Sync)>,
	pub needs: Unlocks,
	pub on: Gesture,
	pub then: Action,
	pub tag: u8,
	/// Only in this stage of the scene (`None`: every stage).
	pub stage: Option<usize>,
}

impl Hotspot {
	/// Whether cell (i, j) is one of the hotspot's.
	pub fn covers(&self, i: usize, j: usize) -> bool {
		self.cells.contains(i, j) && self.inside.map_or(true, |inside| inside(i, j))
	}
}

pub enum Gesture {
	/// Press and release inside `cells`.
	Tap,
	/// Press inside `cells`, release inside `to`.
	Drag { to: Area },
	/// Draw the outline of `cells` (through the centres of its perimeter cells) in one
	/// stroke. The stroke locks in while it's on the outline and fires on completion.
	Outline,
}

pub enum Action {
	/// Switch scene at the next bar.
	Go(&'static str),
	/// A transition: from the next bar, play these bars of the *current* track once while
	/// the current scene intensifies (see `Model::transition`), then switch scene.
	GoVia(&'static str, Range<usize>),
	/// Immediately.
	Unlock(Unlock),
	/// Another stage of the same scene, at the next bar; the music goes on.
	Stage(usize),
	All(&'static [Action]),
}

/// Things the player can learn to see. Global across the album. The owner's system: a cell
/// is dots, lines and rings; each of those exists, then has parameters, each its own small
/// unlock — a player needn't notice getting one. A static look is free (a line points where
/// its layer says from the start); what an unlock gates is the thing, or the parameter
/// leaving its default, or its motion.
///
/// Add freely; the discriminant is the bit in saved progress, so don't reorder existing
/// ones — `ALL` is the display order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unlock {
	/// Points become lines, pointing wherever the layer says.
	Lines = 0,
	/// Lines get a length other than one grid interval.
	Length,
	/// Lines get to turn (the bit that used to be "rotation").
	Angle,
	/// Rings appear (the bit that used to be "circles").
	Rings,
	/// The one color (see `COLOR`). More colors can be more unlocks.
	Color,
	/// Brightness follows the music where a glow layer says (see `glow`). The owner's
	/// "opacity".
	Glow,
	/// Lines bend into arcs.
	Bend,
	/// Rings get a size other than the default.
	Size,
	/// Rings fill in.
	Fill,
	/// Rings morph toward squares.
	Morph,
	/// Rings flatten.
	Flatness,
	/// Rings get to turn.
	Rotation,
}

impl Unlock {
	pub const COUNT: usize = 12;

	/// Display order.
	pub const ALL: [Unlock; Unlock::COUNT] = [
		Unlock::Glow,
		Unlock::Lines,
		Unlock::Length,
		Unlock::Angle,
		Unlock::Bend,
		Unlock::Rings,
		Unlock::Size,
		Unlock::Fill,
		Unlock::Morph,
		Unlock::Flatness,
		Unlock::Rotation,
		Unlock::Color,
	];

	pub fn name(self) -> &'static str {
		match self {
			Unlock::Lines => "lines",
			Unlock::Length => "length",
			Unlock::Angle => "angle",
			Unlock::Rings => "rings",
			Unlock::Color => "color",
			Unlock::Glow => "glow",
			Unlock::Bend => "bend",
			Unlock::Size => "size",
			Unlock::Fill => "fill",
			Unlock::Morph => "morph",
			Unlock::Flatness => "flatness",
			Unlock::Rotation => "rotation",
		}
	}
}

/// A set of unlocks.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Unlocks(pub u64);

pub const NONE: Unlocks = Unlocks(0);

impl Unlocks {
	pub const fn of(list: &[Unlock]) -> Unlocks {
		let mut bits = 0u64;
		let mut k = 0;
		while k < list.len() {
			bits |= 1 << list[k] as u64;
			k += 1;
		}
		Unlocks(bits)
	}

	pub fn has(self, unlock: Unlock) -> bool {
		self.0 & (1 << unlock as u64) != 0
	}

	pub fn has_all(self, needs: Unlocks) -> bool {
		self.0 & needs.0 == needs.0
	}

	pub fn with(self, unlock: Unlock) -> Unlocks {
		Unlocks(self.0 | (1 << unlock as u64))
	}

	pub fn without(self, unlock: Unlock) -> Unlocks {
		Unlocks(self.0 & !(1 << unlock as u64))
	}

	pub fn names(self) -> String {
		Unlock::ALL
			.iter()
			.filter(|&&unlock| self.has(unlock))
			.map(|unlock| unlock.name())
			.collect::<Vec<_>>()
			.join("+")
	}
}

/// A rectangle of grid cells i0..i1 × j0..j1 (end exclusive). j counts up from the bottom.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Area {
	pub i0: usize,
	pub j0: usize,
	pub i1: usize,
	pub j1: usize,
}

pub const fn area(i0: usize, j0: usize, i1: usize, j1: usize) -> Area {
	Area { i0, j0, i1, j1 }
}

/// A unit step between two neighbouring cell centres, normalised so `a <= b`.
pub type Edge = ((usize, usize), (usize, usize));

pub fn edge(a: (usize, usize), b: (usize, usize)) -> Edge {
	if a <= b {
		(a, b)
	} else {
		(b, a)
	}
}

impl Area {
	pub fn contains(&self, i: usize, j: usize) -> bool {
		self.i0 <= i && i < self.i1 && self.j0 <= j && j < self.j1
	}

	/// Inside, and on the outermost ring of cells.
	pub fn on_perimeter(&self, i: usize, j: usize) -> bool {
		self.contains(i, j)
			&& (i == self.i0 || i + 1 == self.i1 || j == self.j0 || j + 1 == self.j1)
	}

	/// The unit edges joining the centres of the perimeter cells, all the way round.
	pub fn perimeter(&self) -> Vec<Edge> {
		let (i0, j0, i1, j1) = (self.i0, self.j0, self.i1 - 1, self.j1 - 1);
		let mut edges = vec![];
		for i in i0..i1 {
			edges.push(((i, j0), (i + 1, j0)));
			if j1 != j0 {
				edges.push(((i, j1), (i + 1, j1)));
			}
		}
		for j in j0..j1 {
			edges.push(((i0, j), (i0, j + 1)));
			if i1 != i0 {
				edges.push(((i1, j), (i1, j + 1)));
			}
		}
		edges
	}
}

/// Every scene: the album (`data`, from `static/album.json`) then the demo graph. The first
/// one is where a new player starts.
pub fn all() -> Vec<&'static Scene> {
	let mut scenes = data::scenes();
	scenes.extend_from_slice(for_against::SCENES);
	scenes.extend_from_slice(terra_firmament::SCENES);
	scenes
}

pub fn find(id: &str) -> Option<&'static Scene> {
	all().into_iter().find(|scene| scene.id == id)
}

/// Scene ids an action can lead to.
pub fn targets(action: &Action, out: &mut Vec<&'static str>) {
	match action {
		Action::Go(id) | Action::GoVia(id, _) => out.push(id),
		Action::Unlock(_) | Action::Stage(_) => {}
		Action::All(list) => list.iter().for_each(|action| targets(action, out)),
	}
}

impl Action {
	/// Nothing left for it to do: it only unlocks things the player already has. (Going
	/// somewhere is never spent.) A spent hotspot disappears, so the same cells can first
	/// unlock something and then, with a second hotspot that `needs` it, lead somewhere.
	pub fn spent(&self, unlocks: Unlocks) -> bool {
		match self {
			Action::Go(_) | Action::GoVia(_, _) | Action::Stage(_) => false,
			Action::Unlock(unlock) => unlocks.has(*unlock),
			// A hotspot with nothing to do yet (the owner places them before deciding) is not
			// spent: its hint, name and hole still show.
			Action::All(list) => !list.is_empty() && list.iter().all(|action| action.spent(unlocks)),
		}
	}
}

/// Unlocks an action grants.
pub fn grants(action: &Action, out: &mut Vec<Unlock>) {
	match action {
		Action::Go(_) | Action::GoVia(_, _) | Action::Stage(_) => {}
		Action::Unlock(unlock) => out.push(*unlock),
		Action::All(list) => list.iter().for_each(|action| grants(action, out)),
	}
}

/// "from -> to" for every scene reference that points nowhere. Checked at startup.
pub fn dangling() -> Vec<String> {
	let all = all();
	let mut out = vec![];
	for scene in &all {
		let mut ids: Vec<&'static str> = vec![];
		for hotspot in scene.hotspots {
			targets(&hotspot.then, &mut ids);
		}
		for id in ids {
			if !all.iter().any(|scene| scene.id == id) {
				out.push(format!("{} -> {id}", scene.id));
			}
		}
	}
	out
}

/// Window coordinates of the centre of cell (i, j).
pub fn scale_coordinates(model: &Model, i: usize, j: usize) -> (f32, f32) {
	// Cell (5, 5) is the centre of the window: the centre dot.
	(
		(i as f32 - (CANVAS_WIDTH - 1) as f32 / 2.0) * model.scaling,
		(j as f32 - (CANVAS_HEIGHT - 1) as f32 / 2.0) * model.scaling,
	)
}

/// The cell whose centre is nearest to window point (x, y), if that's on the grid.
pub fn cell_at(model: &Model, x: f32, y: f32) -> Option<(usize, usize)> {
	let scaling = model.scaling;
	let i = (x / scaling + (CANVAS_WIDTH - 1) as f32 / 2.0).round();
	let j = (y / scaling + (CANVAS_HEIGHT - 1) as f32 / 2.0).round();
	(i >= 0.0 && j >= 0.0 && i < CANVAS_WIDTH as f32 && j < CANVAS_HEIGHT as f32)
		.then(|| (i as usize, j as usize))
}

/// The one color there is, for now. A placeholder: the owner's call.
pub const COLOR: (f32, f32, f32) = (1.0, 0.45, 0.2);

/// Grey at this brightness, warmed toward `COLOR` by `color` (0 = grey, 1 = the color) —
/// once color is unlocked. Scenes pick `color` per cell, so the whole range from white to
/// the color is theirs to use.
pub fn paint(model: &Model, gray: f32, color: f32) -> Rgb {
	tint(gray, color * model.reveal(Unlock::Color))
}

/// `paint` as given, whatever is unlocked (the data renderer gates color itself).
pub fn tint(gray: f32, color: f32) -> Rgb {
	let c = color.clamp(0.0, 1.0);
	let (r, g, b) = COLOR;
	rgb(
		gray * (1.0 + (r - 1.0) * c),
		gray * (1.0 + (g - 1.0) * c),
		gray * (1.0 + (b - 1.0) * c),
	)
}

/// Something a cell draws. Scenes make these with `mark` and `ring`; they're recorded
/// rather than drawn straight away so that, on the way into another scene, each cell's
/// marks can be morphed into what that scene draws there (see `Model::morph`, `blend`).
#[derive(Clone, Copy, Debug)]
pub struct Mark {
	pub x: f32,
	pub y: f32,
	pub shape: Shape,
	pub color: Rgb,
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
	/// From (x, y): `length` px at `angle` radians anticlockwise from right. `bend` curves
	/// it into an arc of that length: 0 straight, 1 a half circle (turning left; negative
	/// turns right).
	Line { angle: f32, length: f32, bend: f32 },
	/// Centred on (x, y): a `radius` px outline `weight` px thick — `fill` (0..1) thickens
	/// it inward until it's a disc, `square` (0..1) morphs it from a circle to a square,
	/// `flat` (0..1) squashes it to nothing across its own `angle`.
	Ring {
		radius: f32,
		weight: f32,
		fill: f32,
		square: f32,
		flat: f32,
		angle: f32,
	},
}

/// The default radius of a ring, in cells: what a ring is until size is unlocked.
pub const RING: f32 = 0.4;
/// A dot's diameter and a line's weight, px (owner, 2026-11-18: 3 px dots; lines thick enough
/// to cover their dot and no more — round caps make the cap the dot).
pub const DOT: f32 = 3.0;
pub const WEIGHT: f32 = 3.0;

impl Mark {
	pub fn is_line(&self) -> bool {
		matches!(self.shape, Shape::Line { .. })
	}

	pub fn draw(&self, draw: &Draw) {
		match self.shape {
			Shape::Line { angle, length, bend } => {
				if length < 0.5 {
					// A dot.
					draw.ellipse().color(self.color).x_y(self.x, self.y).radius(DOT / 2.0);
				} else if bend.abs() < 1e-3 {
					draw.line().weight(WEIGHT).caps_round().color(self.color).points(
						pt2(self.x, self.y),
						pt2(self.x + length * angle.cos(), self.y + length * angle.sin()),
					);
				} else {
					// An arc of this length turning `bend` half-turns, tangent to `angle` at
					// the start.
					let theta = bend * PI;
					let r = length / theta;
					let (cx, cy) = (self.x - r * angle.sin(), self.y + r * angle.cos());
					const N: usize = 16;
					let points = (0..=N).map(|k| {
						let a = angle - PI / 2.0 + theta * k as f32 / N as f32;
						pt2(cx + r * a.cos(), cy + r * a.sin())
					});
					draw.polyline().weight(WEIGHT).caps_round().color(self.color).points(points);
				}
			}
			Shape::Ring {
				radius,
				weight,
				fill,
				square,
				flat,
				angle,
			} => {
				if radius > 0.0 {
					// A closed polygon between the circle and the square of this radius,
					// squashed, turned. The outer edge stays at `radius` as the fill thickens
					// the stroke inward.
					const N: usize = 48;
					let fill = fill.clamp(0.0, 1.0);
					let solid = fill >= 0.999;
					let thick = weight + (radius - weight).max(0.0) * fill;
					let scale = if solid { 1.0 } else { (radius - thick / 2.0).max(0.0) / radius };
					let (sin, cos) = angle.sin_cos();
					let points = (0..N).map(|k| {
						let t = TAU * k as f32 / N as f32;
						let (st, ct) = t.sin_cos();
						let m = ct.abs().max(st.abs());
						let (px, py) = (ct + (ct / m - ct) * square, st + (st / m - st) * square);
						let (px, py) = (px * radius * scale, py * radius * scale * (1.0 - flat.clamp(0.0, 1.0)));
						pt2(self.x + px * cos - py * sin, self.y + px * sin + py * cos)
					});
					if solid {
						draw.polygon().color(self.color).points(points);
					} else {
						draw.polygon()
							.no_fill()
							.stroke_weight(thick)
							.stroke_color(self.color)
							.points(points);
					}
				}
			}
		}
	}

	/// This mark shrunk to nothing where it is.
	fn nothing(&self) -> Mark {
		Mark {
			shape: match self.shape {
				Shape::Line { angle, bend, .. } => Shape::Line {
					angle,
					length: 0.0,
					bend,
				},
				Shape::Ring {
					weight,
					fill,
					square,
					flat,
					angle,
					..
				} => Shape::Ring {
					radius: 0.0,
					weight,
					fill,
					square,
					flat,
					angle,
				},
			},
			color: rgb(0.0, 0.0, 0.0),
			..*self
		}
	}

	/// `t` (0..1) of the way from `a` to `b`. Lines turn the short way round. A missing
	/// side is the other mark shrunk to nothing, so an unpaired mark grows in or shrinks
	/// away in place.
	pub fn between(a: Option<&Mark>, b: Option<&Mark>, t: f32) -> Option<Mark> {
		let (a, b) = match (a, b) {
			(Some(a), Some(b)) => (*a, *b),
			(Some(a), None) => (*a, a.nothing()),
			(None, Some(b)) => (b.nothing(), *b),
			(None, None) => return None,
		};
		let lerp = |p: f32, q: f32| p + (q - p) * t;
		// Angles turn the short way round.
		let swing = |a0: f32, a1: f32| {
			let mut turn = (a1 - a0).rem_euclid(TAU);
			if turn > PI {
				turn -= TAU;
			}
			a0 + turn * t
		};
		let shape = match (a.shape, b.shape) {
			(
				Shape::Line {
					angle: a0,
					length: l0,
					bend: b0,
				},
				Shape::Line {
					angle: a1,
					length: l1,
					bend: b1,
				},
			) => Shape::Line {
				angle: swing(a0, a1),
				length: lerp(l0, l1),
				bend: lerp(b0, b1),
			},
			(
				Shape::Ring {
					radius: r0,
					weight: w0,
					fill: f0,
					square: s0,
					flat: q0,
					angle: a0,
				},
				Shape::Ring {
					radius: r1,
					weight: w1,
					fill: f1,
					square: s1,
					flat: q1,
					angle: a1,
				},
			) => Shape::Ring {
				radius: lerp(r0, r1),
				weight: lerp(w0, w1),
				fill: lerp(f0, f1),
				square: lerp(s0, s1),
				flat: lerp(q0, q1),
				angle: swing(a0, a1),
			},
			// `blend` pairs like with like; should that ever not hold, cut over halfway.
			_ => {
				if t < 0.5 {
					a.shape
				} else {
					b.shape
				}
			}
		};
		Some(Mark {
			x: lerp(a.x, b.x),
			y: lerp(a.y, b.y),
			shape,
			color: rgb(
				lerp(a.color.red, b.color.red),
				lerp(a.color.green, b.color.green),
				lerp(a.color.blue, b.color.blue),
			),
		})
	}
}

/// One cell's marks `t` of the way from what one scene draws there to what another does:
/// lines pair with lines in order, rings with rings; whatever is left over grows in or
/// shrinks away.
pub fn blend(from: &[Mark], to: &[Mark], t: f32) -> Vec<Mark> {
	let mut out = vec![];
	for lines in [true, false] {
		let from: Vec<&Mark> = from.iter().filter(|mark| mark.is_line() == lines).collect();
		let to: Vec<&Mark> = to.iter().filter(|mark| mark.is_line() == lines).collect();
		for k in 0..from.len().max(to.len()) {
			out.extend(Mark::between(from.get(k).copied(), to.get(k).copied(), t));
		}
	}
	out
}

/// A cell's mark, as far as the player can see it. (x, y) is the cell centre; angles are
/// radians anticlockwise from right.
///
/// - no unlocks: a point
/// - Lines: a line one grid interval long at `angle`, which holds still
/// - Length: its length is `length` px instead
/// - Angle: it moves: `turn` is added to the angle (so pass the time-varying part there)
/// - Bend (`mark_bent`): it curves, `bend` half-turns over its length
/// - Color: it's warmed toward `COLOR` by `color` (0..1, see `paint`)
///
/// Each of these eases in over a couple of seconds from the moment it's unlocked (see
/// `Model::reveal`): the line grows out of the point, stretches to its length, winds up
/// to speed, takes on color.
///
/// Recorded, not drawn (see `Mark`); `draw` is here so scenes read naturally.
pub fn mark(
	draw: &Draw,
	model: &Model,
	x: f32,
	y: f32,
	angle: f32,
	turn: f32,
	length: f32,
	gray: f32,
	color: f32,
) {
	mark_bent(draw, model, x, y, angle, turn, length, 0.0, gray, color);
}

/// `mark`, with `bend` (0 straight .. 1 a half circle) once bend is unlocked.
pub fn mark_bent(
	_draw: &Draw,
	model: &Model,
	x: f32,
	y: f32,
	angle: f32,
	turn: f32,
	length: f32,
	bend: f32,
	gray: f32,
	color: f32,
) {
	let color = paint(model, gray, color);
	let lines = model.reveal(Unlock::Lines);
	let shape = if lines == 0.0 {
		// A point: a line of no length draws as the dot.
		Shape::Line {
			angle: 0.0,
			length: 0.0,
			bend: 0.0,
		}
	} else {
		let unit = model.scaling;
		Shape::Line {
			angle: angle + turn * model.reveal(Unlock::Angle),
			length: (unit + (length - unit) * model.reveal(Unlock::Length)) * lines,
			bend: bend * model.reveal(Unlock::Bend),
		}
	};
	model.record(Mark { x, y, shape, color });
}

/// A line the scene insists on, whatever the player has unlocked — scenery (stripes, a
/// frame) rather than a cell's mark. Recorded like `mark`, so it morphs too.
pub fn stroke(_draw: &Draw, model: &Model, x: f32, y: f32, angle: f32, length: f32, color: Rgb) {
	model.record(Mark {
		x,
		y,
		shape: Shape::Line {
			angle,
			length,
			bend: 0.0,
		},
		color,
	});
}

/// A bare point, whatever the player has unlocked (a `mark` becomes a line). Scenery.
pub fn dot(draw: &Draw, model: &Model, x: f32, y: f32, color: Rgb) {
	stroke(draw, model, x, y, 0.0, 0.0, color);
}

/// An event, the way music has them: at `phase` 0 it hits, then it falls away before the
/// next one — a maximally slanted sawtooth with a curve to it, never a sine. `fall` above
/// 1 falls faster.
pub fn hit(phase: f32, fall: f32) -> f32 {
	(1.0 - phase.clamp(0.0, 1.0)).powf(fall)
}

/// The music's pulse, 0..1: every beat a hit that falls to nothing before the next, the
/// downbeat the hardest. What the glow follows. (The place to tie it to the bass band.)
pub fn pulse(modifiers: &[f32; 8]) -> f32 {
	let [whole, _half, quarter, ..] = *modifiers;
	hit(quarter, 2.5) * (0.7 + 0.3 * hit(whole, 1.5))
}

/// A plain ring centred on (x, y). Recorded like `mark`. Scenes gate it on `Unlock::Rings`
/// themselves (usually by scaling `radius` with `model.reveal(Unlock::Rings)`); the data
/// renderer's `rings` kind goes through `shape` with the ring unlocks applied.
pub fn ring(draw: &Draw, model: &Model, x: f32, y: f32, radius: f32, weight: f32, color: Rgb) {
	shape(
		draw,
		model,
		x,
		y,
		Shape::Ring {
			radius,
			weight,
			fill: 0.0,
			square: 0.0,
			flat: 0.0,
			angle: 0.0,
		},
		color,
	);
}

/// Any shape at (x, y), as given. Recorded like `mark`.
pub fn shape(_draw: &Draw, model: &Model, x: f32, y: f32, shape: Shape, color: Rgb) {
	model.record(Mark { x, y, shape, color });
}

fn u8_to_f32(n: u8) -> f32 {
	n as f32 / 255.0
}

/// Fold the bar phase (0..1) into [whole, half, quarter, eighth, sixteenth] phases.
pub fn fold(whole: f32) -> [f32; 5] {
	let half = (whole % 0.5) * 2.0;
	let quarter = (half % 0.5) * 2.0;
	let eighth = (quarter % 0.5) * 2.0;
	let sixteenth = (eighth % 0.5) * 2.0;
	[whole, half, quarter, eighth, sixteenth]
}

pub fn get_modifiers(model: &Model) -> [f32; 8] {
	let [_, half, quarter, eighth, sixteenth] = fold(model.last_whole as f32);

	let fft = model.audio.fft();

	let bass = (u8_to_f32(fft[0]) + u8_to_f32(fft[1])) as f32 / 2.0;
	let mid = (u8_to_f32(fft[2])
		+ u8_to_f32(fft[3])
		+ u8_to_f32(fft[4])
		+ u8_to_f32(fft[5])
		+ u8_to_f32(fft[6])
		+ u8_to_f32(fft[7])) as f32
		/ 6.0;
	let treble = (u8_to_f32(fft[8])
		+ u8_to_f32(fft[9])
		+ u8_to_f32(fft[10])
		+ u8_to_f32(fft[11])
		+ u8_to_f32(fft[12])
		+ u8_to_f32(fft[13])
		+ u8_to_f32(fft[14])
		+ u8_to_f32(fft[15])) as f32
		/ 8.0;

	[
		model.last_whole as f32,
		half,
		quarter,
		eighth,
		sixteenth,
		bass,
		mid,
		treble,
	]
}

/// Beat subdivisions. Index into `Model::ticks` and `modifiers[0..5]`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Subdivision {
	Whole = 0,
	Half,
	Quarter,
	Eighth,
	Sixteenth,
}

/// An ordered partition of the canvas into regions, so an effect can walk across the
/// screen one region per tick: this 16th note happens *here*, the next one *there*.
///
/// ```ignore
/// const SQUARES: Regions = Regions::Grid { cols: 4, rows: 4 };
/// let hot = SQUARES.is_current(model, Subdivision::Sixteenth, i, j);
/// ```
pub enum Regions {
	/// `cols × rows` equal rectangles in reading order (left→right, top→bottom).
	Grid { cols: usize, rows: usize },
	/// Hand-drawn: `CANVAS_HEIGHT` rows of `CANVAS_WIDTH` characters, top row first,
	/// whitespace ignored. `0`–`9` then `a`–`z` give the region order; `.` is no region.
	Map(&'static str),
	/// Anything: cell → region index (must be `< len`), or `None` for no region.
	Cells {
		len: usize,
		of: &'static dyn Fn(usize, usize) -> Option<usize>,
	},
}

impl Regions {
	pub fn len(&self) -> usize {
		match self {
			Regions::Grid { cols, rows } => cols * rows,
			Regions::Map(map) => map
				.chars()
				.filter_map(map_index)
				.max()
				.map_or(0, |max| max + 1),
			Regions::Cells { len, .. } => *len,
		}
	}

	/// Which region cell `(i, j)` belongs to.
	pub fn of(&self, i: usize, j: usize) -> Option<usize> {
		// j counts up from the bottom of the canvas; regions are described top-down.
		let row_from_top = CANVAS_HEIGHT - 1 - j;
		match self {
			Regions::Grid { cols, rows } => {
				Some((row_from_top * rows / CANVAS_HEIGHT) * cols + i * cols / CANVAS_WIDTH)
			}
			Regions::Map(map) => map
				.chars()
				.filter(|c| !c.is_whitespace())
				.nth(row_from_top * CANVAS_WIDTH + i)
				.and_then(map_index),
			Regions::Cells { of, .. } => of(i, j),
		}
	}

	/// The region that is current at tick `step` (wraps around).
	pub fn at(&self, step: usize) -> usize {
		step % self.len().max(1)
	}

	/// Is cell `(i, j)` inside the region that this subdivision's tick count lands on?
	pub fn is_current(&self, model: &Model, sub: Subdivision, i: usize, j: usize) -> bool {
		self.of(i, j) == Some(self.at(model.ticks(sub)))
	}
}

fn map_index(c: char) -> Option<usize> {
	match c {
		'0'..='9' => Some(c as usize - '0' as usize),
		'a'..='z' => Some(10 + c as usize - 'a' as usize),
		_ => None,
	}
}
