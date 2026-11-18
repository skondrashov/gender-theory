//! The album as data: `static/album.json`, written by the editor (`/editor.html`).
//!
//! Everything is measured in bars of a track. A track has **scenes** — stretches of it:
//! `bars` plays once, then from `loop` to the end repeats until the player leaves. A scene
//! has **stages** (visual states, tapped through without the music changing), **layers**
//! (a region of the grid, a mechanic, its parameters, optionally the bars it's live for —
//! the whole scene otherwise — and optionally one stage) and **hotspots** (a region, a
//! gesture, what it needs, what it does, optionally one stage). A scene draws every layer of its own that is live at the bar the
//! music is on and in the current stage, in order.
//!
//! Numbers that move are **params**: a base value plus any number of **sidechains**, each
//! taking a source — one of the album's frequency **bands** (`Album::bands`, followed with
//! attack and release in `Model`: its level as `id`, and where in its range the loudest
//! thing sits as `id.pitch` and `id.chroma`) or a built-in signal (beat, bar, pulse, noise,
//! transition, and the old bass/mid/treble) — through threshold, width, curve and depth.
//!
//! `CATALOGUE` tells the editor what kinds and params exist; the structs here are what
//! they mean. Keep the two together.

use {
	crate::{
		model::{Model, CANVAS_HEIGHT, CANVAS_WIDTH},
		scenes::*,
	},
	serde::Deserialize,
	serde_json::Value,
	std::{cell::RefCell, collections::HashMap},
};

// ---- the file ----

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Album {
	pub bands: Vec<Band>,
	pub tracks: Vec<TrackData>,
	/// The editor's layer presets (`{id, name, layer}`): a library to drop into any scene.
	/// The engine doesn't read them.
	pub presets: Vec<Value>,
}

/// A frequency range of the music, followed as a 0..1 level (source `id`), and as a pitch:
/// where in the range the loudest thing is, on a log scale 0..1 (`id.pitch`), and that
/// folded to a pitch class, C = 0 .. B = 11/12 (`id.chroma`). The pitch holds while the
/// band is below its `floor`, so it doesn't wander in the gaps between notes.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Band {
	pub id: String,
	pub name: String,
	/// Hz.
	pub lo: f32,
	pub hi: f32,
	/// Seconds the follower takes to rise / fall (time constants). The pitch glides with
	/// `attack`.
	pub attack: f32,
	pub release: f32,
	/// Level multiplier (high bands are quiet).
	pub gain: f32,
	/// Level (after gain) under which the band is silent: its pitch holds.
	pub floor: f32,
}

impl Default for Band {
	fn default() -> Self {
		Band {
			id: String::new(),
			name: String::new(),
			lo: 20.0,
			hi: 200.0,
			attack: 0.01,
			release: 0.15,
			gain: 1.0,
			floor: 0.1,
		}
	}
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct TrackData {
	pub id: String,
	pub path: String,
	pub bpm: f64,
	pub scenes: Vec<SceneData>,
}

#[derive(Deserialize)]
pub struct SceneData {
	pub id: String,
	pub bars: [usize; 2],
	/// The bar the tail loop starts on; the first bar if missing (the whole stretch loops).
	#[serde(rename = "loop")]
	pub loop_from: Option<usize>,
	#[serde(default)]
	pub stages: Vec<String>,
	#[serde(default)]
	pub layers: Vec<Layer>,
	#[serde(default)]
	pub hotspots: Vec<HotspotData>,
}

/// Which cells. Everything given must hold: a rectangle, a map (`#` cells of
/// `CANVAS_HEIGHT` rows of `CANVAS_WIDTH` characters, top row first), every `step`th row
/// or column from `phase`, one colour of the checkerboard, the perimeter of the rectangle.
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct Region {
	pub rect: Option<[usize; 4]>,
	pub map: Option<String>,
	pub rows: Option<[usize; 2]>,
	pub cols: Option<[usize; 2]>,
	pub checker: Option<usize>,
	pub perimeter: bool,
}

impl Region {
	pub fn area(&self) -> Area {
		self.rect
			.map(|[i0, j0, i1, j1]| area(i0, j0, i1, j1))
			.unwrap_or(area(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT))
	}

	/// Nothing given: the whole grid.
	pub fn is_unset(&self) -> bool {
		self.rect.is_none()
			&& self.map.is_none()
			&& self.rows.is_none()
			&& self.cols.is_none()
			&& self.checker.is_none()
			&& !self.perimeter
	}

	/// The tight rectangle round the cells it contains; empty (0, 0, 0, 0) if none.
	pub fn bounds(&self) -> Area {
		let mut bounds: Option<Area> = None;
		for i in 0..CANVAS_WIDTH {
			for j in 0..CANVAS_HEIGHT {
				if self.contains(i, j) {
					bounds = Some(match bounds {
						None => area(i, j, i + 1, j + 1),
						Some(b) => area(b.i0.min(i), b.j0.min(j), b.i1.max(i + 1), b.j1.max(j + 1)),
					});
				}
			}
		}
		bounds.unwrap_or(area(0, 0, 0, 0))
	}

	pub fn contains(&self, i: usize, j: usize) -> bool {
		let rect = self.area();
		if !rect.contains(i, j) || (self.perimeter && !rect.on_perimeter(i, j)) {
			return false;
		}
		if let Some(map) = &self.map {
			if at(map, i, j) != '#' {
				return false;
			}
		}
		if let Some([step, phase]) = self.rows {
			if step > 1 && j % step != phase % step {
				return false;
			}
		}
		if let Some([step, phase]) = self.cols {
			if step > 1 && i % step != phase % step {
				return false;
			}
		}
		if let Some(parity) = self.checker {
			if (i + j) % 2 != parity % 2 {
				return false;
			}
		}
		true
	}
}

/// The character of a map at cell (i, j); `.` off the map.
fn at(map: &str, i: usize, j: usize) -> char {
	map.chars()
		.filter(|c| !c.is_whitespace())
		.nth((CANVAS_HEIGHT - 1 - j) * CANVAS_WIDTH + i)
		.unwrap_or('.')
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Layer {
	pub id: String,
	pub name: String,
	/// The bars it's live for; missing: the whole scene.
	pub bars: Option<[usize; 2]>,
	pub region: Region,
	pub kind: String,
	pub params: Value,
	/// Only while a hotspot of the track is (in)visible: hints.
	pub when: Option<When>,
	/// Brightness follows an unlock coming in: `to` 0 fades out as it does, 1 fades in.
	pub fade: Option<Fade>,
	/// Brightness is the product of these unlocks' reveals (0 without them).
	pub needs: Vec<String>,
	/// Draw it at its own gray, whatever glow layers cover the cell.
	pub ignore_glow: bool,
	/// Only in this stage of the scene (by name).
	pub stage: Option<String>,
	pub muted: bool,
}

impl Default for Layer {
	fn default() -> Self {
		Layer {
			id: String::new(),
			name: String::new(),
			bars: None,
			region: Region::default(),
			kind: "marks".into(),
			params: Value::Null,
			when: None,
			fade: None,
			needs: vec![],
			ignore_glow: false,
			stage: None,
			muted: false,
		}
	}
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct When {
	pub hotspot: String,
	pub visible: bool,
}

impl Default for When {
	fn default() -> Self {
		When {
			hotspot: String::new(),
			visible: true,
		}
	}
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
pub struct Fade {
	pub unlock: String,
	pub to: f32,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct HotspotData {
	pub id: String,
	pub name: String,
	/// Unused by the engine for now (a hotspot is there for the whole scene); kept for the
	/// editor's timeline.
	pub bars: Option<[usize; 2]>,
	/// Its cells, like a layer's. The outline gesture traces the rectangle round them.
	pub region: Region,
	/// The old form: just a rectangle. Read when `region` is unset.
	pub rect: Option<[usize; 4]>,
	pub needs: Vec<String>,
	pub on: OnData,
	pub then: Vec<ActionData>,
	pub stage: Option<String>,
}

impl Default for HotspotData {
	fn default() -> Self {
		HotspotData {
			id: String::new(),
			name: String::new(),
			bars: None,
			region: Region::default(),
			rect: None,
			needs: vec![],
			on: OnData::Name("tap".into()),
			then: vec![],
			stage: None,
		}
	}
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum OnData {
	Name(String),
	Drag { drag: [usize; 4] },
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ActionData {
	Go(String),
	Via(String, [usize; 2]),
	Unlock(String),
	Stage(String),
}

// ---- params and sidechains ----

/// A number that can move: `base` plus every sidechain that is on.
#[derive(Clone, Default, Debug)]
pub struct Param {
	pub base: f32,
	pub chains: Vec<Sidechain>,
}

/// One source driving a param: the source's level (0..1) goes through a threshold and a
/// width (a smooth step from `threshold` to `threshold + width`), a curve (power), is
/// inverted if asked, and adds `depth` times that.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Sidechain {
	pub source: String,
	pub depth: f32,
	pub threshold: f32,
	pub width: f32,
	pub curve: f32,
	pub invert: bool,
	pub on: bool,
	/// The source arrives later the farther the cell is from the centre: `delay` × the cell's
	/// distance (0 centre .. 1 corner), in cycles of the source. For beat, bar, pulse, span,
	/// transition — the sources that are a phase. A ripple.
	pub delay: f32,
}

impl Default for Sidechain {
	fn default() -> Self {
		Sidechain {
			source: "beat".into(),
			depth: 1.0,
			threshold: 0.0,
			width: 1.0,
			curve: 1.0,
			invert: false,
			on: true,
			delay: 0.0,
		}
	}
}

impl Sidechain {
	pub fn shape(&self, level: f32) -> f32 {
		let t = ((level - self.threshold) / self.width.max(1e-6)).clamp(0.0, 1.0);
		let t = t * t * (3.0 - 2.0 * t);
		let t = if self.curve > 0.0 && self.curve != 1.0 { t.powf(self.curve) } else { t };
		if self.invert {
			1.0 - t
		} else {
			t
		}
	}
}

/// A param in the file is a bare number or `{base, chains}`.
#[derive(Deserialize)]
#[serde(untagged)]
enum ParamJson {
	Number(f32),
	Full {
		#[serde(default)]
		base: f32,
		#[serde(default)]
		chains: Vec<Sidechain>,
	},
}

impl<'de> Deserialize<'de> for Param {
	fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		Ok(match ParamJson::deserialize(d)? {
			ParamJson::Number(base) => Param { base, chains: vec![] },
			ParamJson::Full { base, chains } => Param { base, chains },
		})
	}
}

impl Param {
	pub fn new(base: f32) -> Param {
		Param { base, chains: vec![] }
	}

	pub fn value(&self, signals: &Signals) -> f32 {
		self.value_gated(signals, 1.0)
	}

	/// `value`, with every sidechain but one on `noise` (which is a fixed pattern, not motion)
	/// scaled by `gate`: how a brightness is read, so it moves only as glow is unlocked.
	pub fn value_gated(&self, signals: &Signals, gate: f32) -> f32 {
		self.base
			+ self
				.chains
				.iter()
				.filter(|chain| chain.on)
				.map(|chain| {
					let fixed = matches!(chain.source.as_str(), "noise" | "distance" | "bearing");
					let gate = if fixed { 1.0 } else { gate };
					gate * chain.depth * chain.shape(signals.get_delayed(&chain.source, chain.delay * signals.distance))
				})
				.sum::<f32>()
	}
}

/// Every source a sidechain can take, as of this frame, this cell and this layer.
#[derive(Clone, Copy)]
pub struct Signals<'a> {
	pub bands: &'a HashMap<String, f32>,
	/// 0..1 across the layer's own bars (the scene's if it has none).
	pub span: f32,
	pub beat: f32,
	pub bar: f32,
	pub pulse: f32,
	pub noise: f32,
	pub transition: f32,
	pub bass: f32,
	pub mid: f32,
	pub treble: f32,
	/// The cell's distance from the centre dot: 0 there, 1 at a corner.
	pub distance: f32,
	/// The cell's direction from the centre dot: 0 straight up, round clockwise to 1.
	pub bearing: f32,
	/// The bar's phase and the beat's, 0..1, for delaying.
	pub whole: f32,
	pub quarter: f32,
}

impl Signals<'_> {
	pub fn get(&self, source: &str) -> f32 {
		match source {
			"span" => self.span,
			"beat" => self.beat,
			"bar" => self.bar,
			"pulse" => self.pulse,
			"noise" => self.noise,
			"transition" => self.transition,
			"distance" => self.distance,
			"bearing" => self.bearing,
			"bass" => self.bass,
			"mid" => self.mid,
			"treble" => self.treble,
			// a band's level, or `id.pitch` / `id.chroma`
			band => self.bands.get(band).copied().unwrap_or(0.0),
		}
	}

	/// The source as it was `shift` cycles ago, for the sources that are a phase (the beat
	/// and bar hits, the pulse, span, transition); the others as they are.
	pub fn get_delayed(&self, source: &str, shift: f32) -> f32 {
		if shift.abs() < 1e-6 {
			return self.get(source);
		}
		let whole = (self.whole - shift).rem_euclid(1.0);
		match source {
			"beat" => hit((self.quarter - shift).rem_euclid(1.0), 2.5),
			"bar" => hit(whole, 1.5),
			"pulse" => {
				let [whole, _, quarter, ..] = fold(whole);
				hit(quarter, 2.5) * (0.7 + 0.3 * hit(whole, 1.5))
			}
			"span" => (self.span - shift).clamp(0.0, 1.0),
			"transition" => (self.transition - shift).clamp(0.0, 1.0),
			_ => self.get(source),
		}
	}
}

// ---- the mechanics ----

/// What the editor needs to know about the kinds and their params. Types: `param` (a base
/// and sidechains), `number`, `bool`, `angle`, `select` (with `options`), `sub`.
pub const CATALOGUE: &str = r##"{
  "sources": ["span", "beat", "bar", "pulse", "noise", "distance", "bearing", "transition", "bass", "mid", "treble"],
  "sourceDocs": {
    "span": "0..1 across the layer's own bars (the scene's if it has none): where we are in it",
    "beat": "hits on every beat and falls away before the next",
    "bar": "hits on the downbeat and falls away over the bar",
    "pulse": "beat and bar together: beat hits, downbeat hardest",
    "noise": "a fixed random value per cell, 0..1",
    "transition": "0..1 along a via passage out of the scene",
    "bass": "the old coarse bands: 0-2.7 kHz",
    "mid": "2.7-11 kHz",
    "treble": "11-22 kHz",
    "distance": "how far the cell is from the centre dot: 0 there, 1 at a corner (fixed per cell; shape it with threshold and width)",
    "bearing": "which way the cell is from the centre dot: 0 straight up, round clockwise to 1 (fixed per cell)"
  },
  "kinds": {
    "marks": {
      "doc": "The cell's mark: a point until lines are unlocked, then a line pointing where `angle` says; its length is 1 until length is unlocked, it turns once angle is, it bends once bend is.",
      "params": {
        "angle": {"type": "angle", "default": "along", "doc": "along / across / noise / checker (along or across by the cell's noise) / around (clockwise round the region's rectangle) / radial (away from the centre dot) / tangent (round the centre dot) / degrees"},
        "offset": {"type": "number", "default": 0, "step": 15, "doc": "degrees added to the angle"},
        "spin": {"type": "param", "default": 0, "doc": "radians per time unit (needs angle)"},
        "alternate": {"type": "bool", "default": false, "doc": "every other cell spins the other way"},
        "length": {"type": "param", "default": 1, "step": 0.1, "doc": "in cells (needs length to be other than 1)"},
        "bend": {"type": "param", "default": 0, "step": 0.1, "doc": "0 straight .. 1 a half circle, turning left; negative turns right (needs bend)"},
        "gray": {"type": "param", "default": 1, "step": 0.05, "doc": "brightness; glow layers over the cell change it, and its own sidechains act once glow is unlocked"},
        "color": {"type": "param", "default": 0, "step": 0.1, "doc": "0 grey .. 1 the color (needs color)"}
      }
    },
    "snake": {
      "doc": "One line through every cell of the region's rectangle, row by row (or column by column), as a snake. retreat: it pulls back cell by cell from its start, leaving a dot at each. arrive: from the dots, each dot in turn extends (and turns) a line to the next until the line is whole. Where it is comes from `progress`: the layer's own bars by default, so put a retreat and an arrive on consecutive bars.",
      "params": {
        "mode": {"type": "select", "options": ["retreat", "arrive"], "default": "retreat"},
        "axis": {"type": "select", "options": ["rows", "cols"], "default": "rows", "doc": "the lines run along rows (left-right) or columns (up-down)"},
        "alternate": {"type": "bool", "default": true, "doc": "every other line runs the opposite way, so it's one continuous snake"},
        "reverse": {"type": "bool", "default": false, "doc": "run the order from the other end"},
        "heads": {"type": "number", "default": 1, "step": 1, "doc": "how many snakes at once (the order cut into equal runs)"},
        "spin": {"type": "number", "default": 0, "step": 15, "doc": "degrees the moving end turns about its dot as it grows or shrinks"},
        "progress": {"type": "param", "default": {"base": 0, "chains": [{"source": "span", "depth": 1, "threshold": 0, "width": 1, "curve": 1, "invert": false, "on": true}]}, "step": 0.05, "doc": "0 untouched .. 1 done, for the mode"},
        "gray": {"type": "param", "default": 0.9, "step": 0.05, "doc": "the line's brightness; sidechains act once glow is unlocked"},
        "dots": {"type": "param", "default": 0.45, "step": 0.05, "doc": "the dots it leaves / starts from, likewise"},
        "color": {"type": "param", "default": 0, "step": 0.1}
      }
    },
    "points": {
      "doc": "Bare points, whatever is unlocked: scenery.",
      "params": {
        "gray": {"type": "param", "default": 0.5, "step": 0.05},
        "color": {"type": "param", "default": 0, "step": 0.1}
      }
    },
    "rings": {
      "doc": "A ring on each cell, once rings are unlocked (it grows in). A circle 0.4 cells across its radius until size is unlocked; each of fill, morph, flatness and rotation does nothing until its own unlock. A line turning inside one is a clock.",
      "params": {
        "radius": {"type": "param", "default": 0.4, "step": 0.05, "doc": "in cells (needs size to be other than 0.4)"},
        "weight": {"type": "number", "default": 3, "step": 0.5, "doc": "stroke, px"},
        "count": {"type": "number", "default": 1, "step": 1, "doc": "how many rings, concentric; the centre dot's halos"},
        "gap": {"type": "number", "default": 1, "step": 0.25, "doc": "cells between them"},
        "fill": {"type": "param", "default": 0, "step": 0.1, "doc": "0 an outline .. 1 a disc (needs fill)"},
        "morph": {"type": "param", "default": 0, "step": 0.1, "doc": "0 a circle .. 1 a square (needs morph)"},
        "flatness": {"type": "param", "default": 0, "step": 0.1, "doc": "0 round .. 1 squashed flat (needs flatness)"},
        "angle": {"type": "number", "default": 0, "step": 15, "doc": "degrees the shape is turned"},
        "spin": {"type": "param", "default": 0, "doc": "radians per time unit (needs rotation)"},
        "gray": {"type": "param", "default": 0.7, "step": 0.05},
        "color": {"type": "param", "default": 0, "step": 0.1},
        "gated": {"type": "bool", "default": true, "doc": "off: scenery, drawn as given whatever is unlocked"}
      }
    },
    "orbits": {
      "doc": "A point orbits each cell and a line runs out to it; a second, invisible point orbits further out the other way, and `pull` drags the tip of the line toward it.",
      "params": {
        "near": {"type": "number", "default": 0.45, "step": 0.05, "doc": "orbit radius, cells"},
        "far": {"type": "number", "default": 0.9, "step": 0.05, "doc": "target radius, cells"},
        "near_rate": {"type": "number", "default": 0.8, "step": 0.1},
        "far_rate": {"type": "number", "default": 1.3, "step": 0.1},
        "vary": {"type": "number", "default": 0.4, "step": 0.1, "doc": "how much the rates vary per cell"},
        "pull": {"type": "param", "default": 0, "step": 0.1, "doc": "0 at the orbit .. 1 at the target"},
        "gray": {"type": "param", "default": 0.6, "step": 0.05},
        "color": {"type": "param", "default": 0, "step": 0.1}
      }
    },
    "glow": {
      "modifier": true,
      "doc": "A mask on brightness: changes the gray of every draw layer under it in this region, once glow is unlocked — nothing until then. Glow layers stack: each multiplies (or adds to) what the ones before it left. Sidechain `level` to breathe over many layers at once (base 0.35 + 0.65 of the pulse is a field, base 1 − 1 of it the inverse); sidechain it to distance, or give the pulse a delay, for ripples.",
      "params": {
        "op": {"type": "select", "options": ["multiply", "add"], "default": "multiply", "doc": "multiply: 1 leaves things alone, 0 is dark; add: 0 leaves things alone, negative darkens"},
        "level": {"type": "param", "default": 1, "step": 0.05}
      }
    },
    "walk": {
      "modifier": true,
      "doc": "A grid of regions lit one per tick of a subdivision, in reading order: this 16th happens here, the next one there. Adds to the gray and length of the marks under it.",
      "params": {
        "cols": {"type": "number", "default": 4, "step": 1},
        "rows": {"type": "number", "default": 4, "step": 1},
        "sub": {"type": "select", "options": ["whole", "half", "quarter", "eighth", "sixteenth"], "default": "sixteenth"},
        "gray": {"type": "number", "default": 0.25, "step": 0.05},
        "length": {"type": "number", "default": 0.8, "step": 0.1, "doc": "cells, falling over the tick"}
      }
    }
  }
}"##;

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum Angle {
	Named(String),
	Degrees(f32),
}

#[derive(Deserialize)]
#[serde(default)]
struct Marks {
	angle: Angle,
	offset: f32,
	spin: Param,
	alternate: bool,
	length: Param,
	bend: Param,
	gray: Param,
	color: Param,
}

impl Default for Marks {
	fn default() -> Self {
		Marks {
			angle: Angle::Named("along".into()),
			offset: 0.0,
			spin: Param::new(0.0),
			alternate: false,
			length: Param::new(1.0),
			bend: Param::new(0.0),
			gray: Param::new(1.0),
			color: Param::new(0.0),
		}
	}
}

#[derive(Deserialize)]
#[serde(default)]
struct Snake {
	mode: String,
	axis: String,
	alternate: bool,
	reverse: bool,
	heads: usize,
	/// Degrees.
	spin: f32,
	progress: Param,
	gray: Param,
	dots: Param,
	color: Param,
}

impl Default for Snake {
	fn default() -> Self {
		Snake {
			mode: "retreat".into(),
			axis: "rows".into(),
			alternate: true,
			reverse: false,
			heads: 1,
			spin: 0.0,
			progress: Param {
				base: 0.0,
				chains: vec![Sidechain {
					source: "span".into(),
					..Default::default()
				}],
			},
			gray: Param::new(0.9),
			dots: Param::new(0.45),
			color: Param::new(0.0),
		}
	}
}

#[derive(Deserialize)]
#[serde(default)]
struct Points {
	gray: Param,
	color: Param,
}

impl Default for Points {
	fn default() -> Self {
		Points {
			gray: Param::new(0.5),
			color: Param::new(0.0),
		}
	}
}

#[derive(Deserialize)]
#[serde(default)]
struct Rings {
	radius: Param,
	weight: f32,
	count: usize,
	gap: f32,
	fill: Param,
	morph: Param,
	flatness: Param,
	/// Degrees.
	angle: f32,
	spin: Param,
	gray: Param,
	color: Param,
	/// Through the ring unlocks; off is scenery.
	gated: bool,
}

impl Default for Rings {
	fn default() -> Self {
		Rings {
			radius: Param::new(RING),
			weight: WEIGHT,
			count: 1,
			gap: 1.0,
			fill: Param::new(0.0),
			morph: Param::new(0.0),
			flatness: Param::new(0.0),
			angle: 0.0,
			spin: Param::new(0.0),
			gray: Param::new(0.7),
			color: Param::new(0.0),
			gated: true,
		}
	}
}

#[derive(Deserialize)]
#[serde(default)]
struct Orbits {
	near: f32,
	far: f32,
	near_rate: f32,
	far_rate: f32,
	vary: f32,
	pull: Param,
	gray: Param,
	color: Param,
}

impl Default for Orbits {
	fn default() -> Self {
		Orbits {
			near: 0.45,
			far: 0.9,
			near_rate: 0.8,
			far_rate: 1.3,
			vary: 0.4,
			pull: Param::new(0.0),
			gray: Param::new(0.6),
			color: Param::new(0.0),
		}
	}
}

#[derive(Deserialize)]
#[serde(default)]
struct Glow {
	/// multiply (1 is nothing) or add (0 is nothing) to the brightness of what's under it.
	op: String,
	level: Param,
}

impl Default for Glow {
	fn default() -> Self {
		Glow {
			op: "multiply".into(),
			level: Param::new(1.0),
		}
	}
}

#[derive(Deserialize)]
#[serde(default)]
struct Walk {
	cols: usize,
	rows: usize,
	sub: String,
	gray: f32,
	length: f32,
}

impl Default for Walk {
	fn default() -> Self {
		Walk {
			cols: 4,
			rows: 4,
			sub: "sixteenth".into(),
			gray: 0.25,
			length: 0.8,
		}
	}
}

enum Kind {
	Marks(Marks),
	Snake(Snake),
	Points(Points),
	Rings(Rings),
	Orbits(Orbits),
	Glow(Glow),
	Walk(Walk),
}

fn parse_kind(kind: &str, params: &Value) -> Result<Kind, String> {
	let params = match params {
		Value::Object(_) => params.clone(),
		_ => Value::Object(Default::default()),
	};
	let bad = |e: serde_json::Error| format!("{kind}: {e}");
	Ok(match kind {
		"marks" => Kind::Marks(serde_json::from_value(params).map_err(bad)?),
		"snake" => Kind::Snake(serde_json::from_value(params).map_err(bad)?),
		"points" => Kind::Points(serde_json::from_value(params).map_err(bad)?),
		"rings" => Kind::Rings(serde_json::from_value(params).map_err(bad)?),
		"orbits" => Kind::Orbits(serde_json::from_value(params).map_err(bad)?),
		"glow" => Kind::Glow(serde_json::from_value(params).map_err(bad)?),
		"walk" => Kind::Walk(serde_json::from_value(params).map_err(bad)?),
		other => return Err(format!("no such kind: {other}")),
	})
}

fn subdivision(name: &str) -> Subdivision {
	match name {
		"whole" => Subdivision::Whole,
		"half" => Subdivision::Half,
		"quarter" => Subdivision::Quarter,
		"eighth" => Subdivision::Eighth,
		_ => Subdivision::Sixteenth,
	}
}

pub fn unlock_named(name: &str) -> Option<Unlock> {
	Unlock::ALL.iter().copied().find(|unlock| unlock.name() == name)
}

/// The direction a cell on the edge of `area` draws in to go round it clockwise.
/// Straight away from the centre dot (right, for the centre itself).
fn radial(i: usize, j: usize) -> f32 {
	let (di, dj) = (
		i as f32 - (CANVAS_WIDTH - 1) as f32 / 2.0,
		j as f32 - (CANVAS_HEIGHT - 1) as f32 / 2.0,
	);
	if di == 0.0 && dj == 0.0 {
		0.0
	} else {
		dj.atan2(di)
	}
}

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

// ---- built scenes ----

struct LayerRt {
	layer: Layer,
	kind: Kind,
	/// Index of the stage it belongs to, if one.
	stage: Option<usize>,
}

struct DataScene {
	layers: &'static [LayerRt],
	/// Hotspot id → its tag in this scene.
	tags: Vec<(String, u8)>,
	/// The scene's bars, for layers without their own.
	bars: [usize; 2],
}

thread_local! {
	static DATA: RefCell<(Vec<&'static Scene>, Vec<Band>)> = RefCell::new((vec![], vec![]));
}

/// The data scenes as last installed, in file order.
pub fn scenes() -> Vec<&'static Scene> {
	DATA.with(|data| data.borrow().0.clone())
}

pub fn bands() -> Vec<Band> {
	DATA.with(|data| data.borrow().1.clone())
}

fn leak<T>(value: T) -> &'static T {
	Box::leak(Box::new(value))
}

fn leak_str(value: &str) -> &'static str {
	Box::leak(value.to_string().into_boxed_str())
}

/// Parse an album and make it the data scenes. Everything is leaked: the album is loaded
/// once, or a few times by the editor.
pub fn install(json: &str) -> Result<usize, String> {
	let album: Album = serde_json::from_str(json).map_err(|e| format!("album.json: {e}"))?;
	let mut scenes: Vec<&'static Scene> = vec![];
	for track in album.tracks {
		let bpm = if track.bpm > 0.0 { track.bpm } else { 120.0 };
		let track_ref: &'static Track = leak(Track {
			id: leak_str(&track.id),
			path: leak_str(&track.path),
			bpm,
		});
		for scene in track.scenes {
			let stage_index = |name: &Option<String>| -> Option<usize> {
				let name = name.as_deref()?;
				scene.stages.iter().position(|stage| stage == name)
			};
			let mut layers = vec![];
			for layer in scene.layers {
				let kind = parse_kind(&layer.kind, &layer.params)
					.map_err(|e| format!("{} layer {:?}: {e}", scene.id, layer.name))?;
				let stage = stage_index(&layer.stage);
				layers.push(LayerRt { layer, kind, stage });
			}
			let layers: &'static [LayerRt] = Vec::leak(layers);
			let bars = scene.bars[0]..scene.bars[1].max(scene.bars[0] + 1);
			let loop_from = scene.loop_from.unwrap_or(bars.start).clamp(bars.start, bars.end - 1);
			let stages: Vec<&'static str> = scene.stages.iter().map(|s| leak_str(s)).collect();
			let mut hotspots = vec![];
			let mut tags = vec![];
			for hotspot in &scene.hotspots {
				let tag = hotspots.len() as u8;
				tags.push((hotspot.id.clone(), tag));
				let region: &'static Region = leak(if hotspot.region.is_unset() {
					Region {
						rect: hotspot.rect,
						..Default::default()
					}
				} else {
					hotspot.region.clone()
				});
				let inside: &'static (dyn Fn(usize, usize) -> bool + Sync) =
					leak(move |i: usize, j: usize| region.contains(i, j));
				let needs = Unlocks::of(
					&hotspot
						.needs
						.iter()
						.filter_map(|name| unlock_named(name))
						.collect::<Vec<_>>(),
				);
				let on = match &hotspot.on {
					OnData::Name(name) if name == "outline" => Gesture::Outline,
					OnData::Name(_) => Gesture::Tap,
					OnData::Drag { drag: [a, b, c, d] } => Gesture::Drag {
						to: area(*a, *b, *c, *d),
					},
				};
				let mut actions: Vec<Action> = hotspot
					.then
					.iter()
					.filter_map(|action| match action {
						ActionData::Go(id) => Some(Action::Go(leak_str(id))),
						ActionData::Via(id, [a, b]) => Some(Action::GoVia(leak_str(id), *a..*b)),
						ActionData::Unlock(name) => unlock_named(name).map(Action::Unlock),
						ActionData::Stage(name) => stage_index(&Some(name.clone())).map(Action::Stage),
					})
					.collect();
				let then = if actions.len() == 1 {
					actions.remove(0)
				} else {
					Action::All(Vec::leak(actions))
				};
				hotspots.push(Hotspot {
					cells: region.bounds(),
					inside: Some(inside),
					needs,
					on,
					then,
					tag,
					stage: stage_index(&hotspot.stage),
				});
			}
			let data: &'static DataScene = leak(DataScene {
				layers,
				tags,
				bars: [bars.start, bars.end],
			});
			let render: &'static dyn Fn(&Draw, &Model, usize, usize, f32, [f32; 8]) =
				leak(move |draw: &Draw, model: &Model, i, j, time, modifiers| {
					render(data, draw, model, i, j, time, modifiers)
				});
			scenes.push(leak(Scene {
				id: leak_str(&scene.id),
				track: track_ref,
				bars,
				loop_from,
				stages: Vec::leak(stages),
				render,
				hotspots: Vec::leak(hotspots),
			}));
		}
	}
	let count = scenes.len();
	DATA.with(|data| *data.borrow_mut() = (scenes, album.bands));
	Ok(count)
}

/// Is this layer showing, and how bright: its `when`, `fade` and `needs`.
fn gate(scene: &DataScene, model: &Model, layer: &Layer) -> Option<f32> {
	if layer.muted {
		return None;
	}
	if let Some(when) = &layer.when {
		let tag = scene
			.tags
			.iter()
			.find(|(id, _)| *id == when.hotspot)
			.map(|(_, tag)| *tag);
		let visible = tag.map_or(false, |tag| model.hint_visible(tag));
		if visible != when.visible {
			return None;
		}
	}
	let mut factor = 1.0;
	if let Some(fade) = &layer.fade {
		if let Some(unlock) = unlock_named(&fade.unlock) {
			let reveal = model.reveal(unlock);
			factor *= if fade.to >= 0.5 { reveal } else { 1.0 - reveal };
		}
	}
	for name in &layer.needs {
		if let Some(unlock) = unlock_named(name) {
			factor *= model.reveal(unlock);
		}
	}
	(factor > 0.0).then_some(factor)
}

/// The glow of a cell at `level` (0..1): 1 until glow is unlocked, easing to the level as
/// it comes in.
fn lit(model: &Model, level: f32) -> f32 {
	1.0 + (level.clamp(0.0, 1.0) - 1.0) * model.reveal(Unlock::Glow)
}

fn render(
	scene: &'static DataScene,
	draw: &Draw,
	model: &Model,
	i: usize,
	j: usize,
	time: f32,
	modifiers: [f32; 8],
) {
	let bar = model.bar();
	let unit = model.scaling;
	let (x, y) = scale_coordinates(model, i, j);
	let raw = model.noisemap.get_value(i, j) as f32;
	let [whole, _half, quarter, _eighth, _sixteenth, bass, mid, treble] = modifiers;
	let pulse = pulse(&modifiers);
	let (di, dj) = (
		i as f32 - (CANVAS_WIDTH - 1) as f32 / 2.0,
		j as f32 - (CANVAS_HEIGHT - 1) as f32 / 2.0,
	);
	let corner = ((CANVAS_WIDTH - 1) as f32 / 2.0).hypot((CANVAS_HEIGHT - 1) as f32 / 2.0);
	let signals = Signals {
		bands: model.bands(),
		span: 0.0,
		beat: hit(quarter, 2.5),
		bar: hit(whole, 1.5),
		pulse,
		noise: raw.abs().min(1.0),
		transition: model.transition().unwrap_or(0.0),
		bass,
		mid,
		treble,
		distance: (di.hypot(dj) / corner).min(1.0),
		bearing: if di == 0.0 && dj == 0.0 { 0.0 } else { di.atan2(dj).rem_euclid(TAU) / TAU },
		whole,
		quarter,
	};
	// The same signals, with `span` where the bar is in this layer's own bars.
	let with_span = |rt: &LayerRt| {
		let [from, to] = rt.layer.bars.unwrap_or(scene.bars);
		let span = if to > from {
			((bar - from as f32) / (to - from) as f32).clamp(0.0, 1.0)
		} else {
			0.0
		};
		Signals { span, ..signals }
	};
	let stage = model.stage();
	let active = || {
		scene.layers.iter().filter(|rt| {
			rt.stage.map_or(true, |s| s == stage)
				&& rt.layer.bars.map_or(true, |[from, to]| from as f32 <= bar && bar < to as f32)
				&& rt.layer.region.contains(i, j)
		})
	};

	// The modifiers first: how bright the cell glows (lit unless a glow layer says), and
	// whether a walk is on it.
	let mut level = 1.0;
	let (mut hot_gray, mut hot_length) = (0.0, 0.0);
	for rt in active() {
		let signals = with_span(rt);
		match &rt.kind {
			Kind::Glow(glow) => {
				if gate(scene, model, &rt.layer).is_some() {
					let value = glow.level.value(&signals);
					level = if glow.op == "add" { level + value } else { level * value };
				}
			}
			Kind::Walk(walk) => {
				let sub = subdivision(&walk.sub);
				let grid = Regions::Grid {
					cols: walk.cols.max(1),
					rows: walk.rows.max(1),
				};
				if gate(scene, model, &rt.layer).is_some() && grid.is_current(model, sub, i, j) {
					let fall = 1.0 - modifiers[sub as usize];
					hot_gray += walk.gray * fall;
					hot_length += walk.length * fall;
				}
			}
			_ => {}
		}
	}

	for rt in active() {
		let Some(factor) = gate(scene, model, &rt.layer) else {
			continue;
		};
		let signals = with_span(rt);
		// Brightness: the layer's own gray, whose sidechains act as glow comes in, dimmed by
		// the glow layers over the cell unless it ignores them.
		let glow = model.reveal(Unlock::Glow);
		let lit = if rt.layer.ignore_glow { 1.0 } else { lit(model, level) } * factor;
		match &rt.kind {
			Kind::Marks(marks) => {
				let angle = match &marks.angle {
					Angle::Degrees(degrees) => degrees.to_radians(),
					Angle::Named(name) => match name.as_str() {
						"across" => PI / 2.0,
						"noise" => raw * TAU,
						"checker" => {
							if (raw * 10.0).floor() as i32 % 2 == 0 {
								0.0
							} else {
								PI / 2.0
							}
						}
						"around" => around(rt.layer.region.area(), i, j),
						"radial" => radial(i, j),
						"tangent" => radial(i, j) + PI / 2.0,
						_ => 0.0,
					},
				};
				let dir = if marks.alternate && (i + j) % 2 == 1 { -1.0 } else { 1.0 };
				let turn = marks.spin.value(&signals) * time * dir;
				let length = (marks.length.value(&signals) + hot_length) * unit;
				let gray = (marks.gray.value_gated(&signals, glow) + hot_gray) * lit;
				mark_bent(
					draw,
					model,
					x,
					y,
					angle + marks.offset.to_radians(),
					turn,
					length,
					marks.bend.value(&signals),
					gray,
					marks.color.value(&signals),
				);
			}
			Kind::Snake(snake) => {
				// One polyline through the cell centres of the region's rectangle in serpentine
				// order; cell k owns the segment from its centre to the next cell's.
				let rect = rt.layer.region.area();
				let (w, h) = (rect.i1.saturating_sub(rect.i0), rect.j1.saturating_sub(rect.j0));
				if w == 0 || h == 0 {
					continue;
				}
				let n = w * h;
				let along_rows = snake.axis != "cols";
				let len = if along_rows { w } else { h };
				let order = |i: usize, j: usize| {
					let (line, pos) = if along_rows {
						(rect.j1 - 1 - j, i - rect.i0)
					} else {
						(i - rect.i0, j - rect.j0)
					};
					let pos = if snake.alternate && line % 2 == 1 { len - 1 - pos } else { pos };
					let k = line * len + pos;
					if snake.reverse { n - 1 - k } else { k }
				};
				let cell = |k: usize| {
					let k = if snake.reverse { n - 1 - k } else { k };
					let (line, pos) = (k / len, k % len);
					let pos = if snake.alternate && line % 2 == 1 { len - 1 - pos } else { pos };
					if along_rows {
						(rect.i0 + pos, rect.j1 - 1 - line)
					} else {
						(rect.i0 + line, rect.j0 + pos)
					}
				};
				let heads = snake.heads.clamp(1, n);
				let run = (n + heads - 1) / heads;
				let k = order(i, j);
				let (head, c) = (k / run, k % run);
				let last = ((head + 1) * run).min(n) - 1;
				let run_len = last + 1 - head * run;
				let front = snake.progress.value(&signals).clamp(0.0, 1.0) * run_len as f32;
				let color = snake.color.value(&signals);
				let dots = (snake.dots.value_gated(&signals, glow) + hot_gray) * lit;
				dot(draw, model, x, y, paint(model, dots, color));
				if k < last {
					let (ni, nj) = cell(k + 1);
					let (qx, qy) = scale_coordinates(model, ni, nj);
					let arrive = snake.mode == "arrive";
					let c = c as f32;
					let fill = if arrive { front - c } else { c + 1.0 - front }.clamp(0.0, 1.0);
					if fill > 0.0 {
						// arriving grows out of this dot towards the next; retreating pulls back
						// from this dot towards the next, anchored there
						let (sx, sy, dx, dy) = if arrive { (x, y, qx - x, qy - y) } else { (qx, qy, x - qx, y - qy) };
						let angle = dy.atan2(dx) + snake.spin.to_radians() * (1.0 - fill);
						let gray = (snake.gray.value_gated(&signals, glow) + hot_gray) * lit;
						stroke(draw, model, sx, sy, angle, fill * unit, paint(model, gray, color));
					}
				}
			}
			Kind::Points(points) => {
				let gray = (points.gray.value_gated(&signals, glow) + hot_gray) * lit;
				dot(draw, model, x, y, paint(model, gray, points.color.value(&signals)));
			}
			Kind::Rings(rings) => {
				// Each parameter sits at its default until its unlock, then eases to the layer's
				// value; the ring itself grows in with `rings`. Scenery (`gated` off) is as given.
				let gate = |value: f32, default: f32, unlock: Unlock| {
					if rings.gated {
						default + (value - default) * model.reveal(unlock)
					} else {
						value
					}
				};
				let grown = gate(1.0, 0.0, Unlock::Rings);
				let radius = (gate(rings.radius.value(&signals), RING, Unlock::Size) + hot_length) * unit * grown;
				let spin = gate(rings.spin.value(&signals), 0.0, Unlock::Rotation) * time;
				let gray = (rings.gray.value_gated(&signals, glow) + hot_gray) * lit;
				let (fill, square, flat) = (
					gate(rings.fill.value(&signals), 0.0, Unlock::Fill),
					gate(rings.morph.value(&signals), 0.0, Unlock::Morph),
					gate(rings.flatness.value(&signals), 0.0, Unlock::Flatness),
				);
				for k in 0..rings.count.max(1) {
					shape(
						draw,
						model,
						x,
						y,
						Shape::Ring {
							radius: radius + k as f32 * rings.gap * unit * grown,
							weight: rings.weight,
							fill,
							square,
							flat,
							angle: rings.angle.to_radians() + spin,
						},
						paint(model, gray, rings.color.value(&signals)),
					);
				}
			}
			Kind::Orbits(orbits) => {
				let near = (orbits.near_rate + orbits.vary * raw) * time + raw * TAU;
				let far = -(orbits.far_rate + orbits.vary * raw) * time + raw * 7.0;
				let (x1, y1) = (
					x + orbits.near * unit * near.cos(),
					y + orbits.near * unit * near.sin(),
				);
				let (x2, y2) = (
					x + orbits.far * unit * far.cos(),
					y + orbits.far * unit * far.sin(),
				);
				let pull = orbits.pull.value(&signals).clamp(0.0, 1.0);
				let (ex, ey) = (x1 + (x2 - x1) * pull, y1 + (y2 - y1) * pull);
				let gray = (orbits.gray.value_gated(&signals, glow) + hot_gray) * lit;
				let color = orbits.color.value(&signals);
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
					gray,
					color,
				);
			}
			Kind::Glow(_) | Kind::Walk(_) => {}
		}
	}
}
