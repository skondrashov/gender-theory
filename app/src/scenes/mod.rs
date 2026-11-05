use {
	crate::model::{Model, CANVAS_HEIGHT, CANVAS_WIDTH},
	nannou::{prelude::*, Draw},
	std::{f32::consts::SQRT_2, ops::Range},
};

// pub mod being_useless;
pub mod for_against;
// pub mod porcelain_piss;
pub mod terra_firmament;

/// One mp3. `measures` is its bar count, counted by hand: the bar grid every scene on it uses.
pub struct Track {
	pub id: &'static str,
	pub path: &'static str,
	pub measures: usize,
}

/// A looped section of a track, a way of seeing it, and the things you can do in it.
pub struct Scene {
	pub id: &'static str,
	pub track: &'static Track,
	/// The looped section, in bars of `track` (end exclusive).
	pub bars: Range<usize>,
	/// Called once per grid cell per frame: (draw, model, i, j, time, modifiers).
	pub render: &'static dyn Fn(&Draw, &Model, usize, usize, f32, [f32; 8]),
	pub hotspots: &'static [Hotspot],
}

/// Something to do in a scene. Invisible and inert until the player has all of `needs`.
/// The scene's render draws the hint itself, by asking `model.hint_at(i, j)` for the `tag`.
pub struct Hotspot {
	pub cells: Area,
	pub needs: Unlocks,
	pub on: Gesture,
	pub then: Action,
	pub tag: u8,
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
	All(&'static [Action]),
}

/// Things the player can learn to see. Global across the album. Add freely; the order only
/// matters for saved progress, so don't reorder existing ones.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unlock {
	/// Points become lines.
	Lines = 0,
	/// Lines get to have a length other than one grid interval.
	Length,
	/// Lines get to point somewhere other than right.
	Rotation,
	Circles,
	/// The one colour (see `COLOR`). More colours can be more unlocks.
	Color,
}

impl Unlock {
	pub const ALL: [Unlock; 5] = [
		Unlock::Lines,
		Unlock::Length,
		Unlock::Rotation,
		Unlock::Circles,
		Unlock::Color,
	];

	pub fn name(self) -> &'static str {
		match self {
			Unlock::Lines => "lines",
			Unlock::Length => "length",
			Unlock::Rotation => "rotation",
			Unlock::Circles => "circles",
			Unlock::Color => "color",
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

/// Every scene in the album, in file order. The first one is where a new player starts.
pub fn all() -> Vec<&'static Scene> {
	let mut scenes = vec![];
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
		Action::Unlock(_) => {}
		Action::All(list) => list.iter().for_each(|action| targets(action, out)),
	}
}

impl Action {
	/// Nothing left for it to do: it only unlocks things the player already has. (Going
	/// somewhere is never spent.) A spent hotspot disappears, so the same cells can first
	/// unlock something and then, with a second hotspot that `needs` it, lead somewhere.
	pub fn spent(&self, unlocks: Unlocks) -> bool {
		match self {
			Action::Go(_) | Action::GoVia(_, _) => false,
			Action::Unlock(unlock) => unlocks.has(*unlock),
			Action::All(list) => list.iter().all(|action| action.spent(unlocks)),
		}
	}
}

/// Unlocks an action grants.
pub fn grants(action: &Action, out: &mut Vec<Unlock>) {
	match action {
		Action::Go(_) | Action::GoVia(_, _) => {}
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
	(
		(i as f32 - (CANVAS_WIDTH as f32 / 2.0) as f32) * model.scaling as f32,
		(j as f32 - (CANVAS_HEIGHT as f32 / 2.0) as f32) * model.scaling as f32,
	)
}

/// The cell whose centre is nearest to window point (x, y), if that's on the grid.
pub fn cell_at(model: &Model, x: f32, y: f32) -> Option<(usize, usize)> {
	let scaling = model.scaling as f32;
	let i = (x / scaling + CANVAS_WIDTH as f32 / 2.0).round();
	let j = (y / scaling + CANVAS_HEIGHT as f32 / 2.0).round();
	(i >= 0.0 && j >= 0.0 && i < CANVAS_WIDTH as f32 && j < CANVAS_HEIGHT as f32)
		.then(|| (i as usize, j as usize))
}

/// The one colour there is, for now. A placeholder: the owner's call.
pub const COLOR: (f32, f32, f32) = (1.0, 0.45, 0.2);

/// Grey at this brightness, warmed toward `COLOR` by `color` (0 = grey, 1 = the colour) —
/// once colour is unlocked. Scenes pick `color` per cell, so the whole range from white to
/// the colour is theirs to use.
pub fn paint(model: &Model, gray: f32, color: f32) -> Rgb {
	let c = (color * model.reveal(Unlock::Color)).clamp(0.0, 1.0);
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
	/// From (x, y): `length` px at `angle` radians anticlockwise from right.
	Line { angle: f32, length: f32 },
	/// Centred on (x, y).
	Ring { radius: f32, weight: f32 },
}

impl Mark {
	pub fn is_line(&self) -> bool {
		matches!(self.shape, Shape::Line { .. })
	}

	pub fn draw(&self, draw: &Draw) {
		match self.shape {
			Shape::Line { angle, length } => {
				draw.line().color(self.color).points(
					pt2(self.x, self.y),
					pt2(self.x + length * angle.cos(), self.y + length * angle.sin()),
				);
			}
			Shape::Ring { radius, weight } => {
				if radius > 0.0 {
					draw.ellipse()
						.no_fill()
						.stroke_weight(weight)
						.stroke_color(self.color)
						.x_y(self.x, self.y)
						.radius(radius);
				}
			}
		}
	}

	/// This mark shrunk to nothing where it is.
	fn nothing(&self) -> Mark {
		Mark {
			shape: match self.shape {
				Shape::Line { angle, .. } => Shape::Line { angle, length: 0.0 },
				Shape::Ring { weight, .. } => Shape::Ring { radius: 0.0, weight },
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
		let shape = match (a.shape, b.shape) {
			(Shape::Line { angle: a0, length: l0 }, Shape::Line { angle: a1, length: l1 }) => {
				let mut turn = (a1 - a0).rem_euclid(TAU);
				if turn > PI {
					turn -= TAU;
				}
				Shape::Line {
					angle: a0 + turn * t,
					length: lerp(l0, l1),
				}
			}
			(Shape::Ring { radius: r0, weight: w0 }, Shape::Ring { radius: r1, weight: w1 }) => {
				Shape::Ring {
					radius: lerp(r0, r1),
					weight: lerp(w0, w1),
				}
			}
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
/// - Rotation: it moves: `turn` is added to the angle (so pass the time-varying part there)
/// - Color: it's warmed toward `COLOR` by `color` (0..1, see `paint`)
///
/// Each of these eases in over a couple of seconds from the moment it's unlocked (see
/// `Model::reveal`): the line grows out of the point, stretches to its length, winds up
/// to speed, takes on colour.
///
/// Recorded, not drawn (see `Mark`); `draw` is here so scenes read naturally.
pub fn mark(
	_draw: &Draw,
	model: &Model,
	x: f32,
	y: f32,
	angle: f32,
	turn: f32,
	length: f32,
	gray: f32,
	color: f32,
) {
	let color = paint(model, gray, color);
	let lines = model.reveal(Unlock::Lines);
	let shape = if lines == 0.0 {
		// A point: the shortest line that shows.
		Shape::Line {
			angle: PI / 4.0,
			length: SQRT_2,
		}
	} else {
		let unit = model.scaling as f32;
		Shape::Line {
			angle: angle + turn * model.reveal(Unlock::Rotation),
			length: (unit + (length - unit) * model.reveal(Unlock::Length)) * lines,
		}
	};
	model.record(Mark { x, y, shape, color });
}

/// A ring centred on (x, y). Recorded like `mark`. Scenes gate it on `Unlock::Circles`
/// themselves (usually by scaling `radius` with `model.reveal(Unlock::Circles)`).
pub fn ring(_draw: &Draw, model: &Model, x: f32, y: f32, radius: f32, weight: f32, color: Rgb) {
	model.record(Mark {
		x,
		y,
		shape: Shape::Ring { radius, weight },
		color,
	});
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
