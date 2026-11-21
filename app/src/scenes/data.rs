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
//! Most layers are **cells**: every cell in the region is a dot with a line out of it and
//! a ring round it, described by thirteen **rows** — one per unlock (`ROWS`): brightness,
//! dots; lines, length, angle, bend; rings, radius, fill, morph, flatness, rotation; color. A
//! cells layer carries the rows it wants to change and, for each, how: set it, add to it
//! or multiply it, over what the layers before it left. Nothing: a dark dot. A row's base
//! is one number or **painted** — a `Grid`: a map of glyphs with a legend, a value per
//! cell, the unpainted cells left alone — so one layer holds a pattern. A row's
//! sidechains add to it; its masks say how much of its change lands — on distance, bearing
//! and around a pattern, on `hotspot:<id>` / `unlock:<name>` a state of the game (a hint
//! shows while its hotspot does), on the music, motion — which comes in with the row's
//! unlock.
//!
//! `catalogue()` tells the editor what rows, kinds and params exist; the structs here are
//! what they mean. Keep the two together.

use super::font;
use crate::names;
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
	/// Stage name → bars its unlocks take to come back in on entering it (`""` for a
	/// stageless scene). See `Scene::delays`.
	#[serde(default)]
	pub unlock_delays: HashMap<String, f32>,
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
			stage: None,
			muted: false,
		}
	}
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

/// A number that can move: `base` plus every sidechain that is on. The base can be
/// **painted** — a `grid`: one value per cell — so one layer carries a pattern (owner,
/// 2026-11-20: "edit complicated line patterns without having to make 4 or more layers for
/// different line orientations"); `base` is then what a cell off the grid's map gets, and
/// a row skips the cells its grid leaves unpainted.
#[derive(Clone, Default, Debug)]
pub struct Param {
	pub base: f32,
	pub chains: Vec<Sidechain>,
	pub grid: Option<Grid>,
}

/// A base painted cell by cell: `map` is a region-style map (11 rows of 11 glyphs, top row
/// first, `.` unpainted) and `legend` what each glyph is worth, so a pattern reads in the
/// file (`→↑←↓` for angles, say). A cell whose glyph is not in the legend is unpainted.
#[derive(Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct Grid {
	pub map: String,
	pub legend: HashMap<String, f32>,
}

impl Grid {
	pub fn at(&self, i: usize, j: usize) -> Option<f32> {
		let c = at(&self.map, i, j);
		if c == '.' {
			return None;
		}
		let mut buf = [0u8; 4];
		self.legend.get(c.encode_utf8(&mut buf) as &str).copied()
	}
}

/// A base in the file: a number, or a painted grid.
#[derive(Deserialize)]
#[serde(untagged)]
enum BaseJson {
	Number(f32),
	Grid(Grid),
}

impl Default for BaseJson {
	fn default() -> Self {
		BaseJson::Number(0.0)
	}
}

impl BaseJson {
	fn split(self) -> (f32, Option<Grid>) {
		match self {
			BaseJson::Number(v) => (v, None),
			BaseJson::Grid(g) => (0.0, Some(g)),
		}
	}
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
	/// The source arrives later the farther the cell is from the centre: `ripple` × the
	/// cell's distance (0 centre .. 1 corner), in cycles of the source. For beat, bar, pulse,
	/// span, transition — the sources that are a phase. (Was `delay` until 2026-11-21 — the
	/// owner: "setting the delay makes the cells desync from each other" — still read.)
	#[serde(alias = "delay")]
	pub ripple: f32,
	/// For beat, bar and their phases: a cycle is `every` beats (bars), counted from the
	/// downbeat and on across bars — 2 every other beat, 3 a three against the four, 0.5
	/// twice a beat — and `offset` says which of them (every 2, offset 1: the off-beats).
	/// Owner, 2026-11-21: "if i want something to happen every 2 beats? or 3?".
	pub every: f32,
	pub offset: f32,
	/// With `source` "position": the field this is, a function of where the cell is
	/// (`fields::FIELDS`), with parameters that are params of their own.
	pub field: Option<fields::Field>,
	/// As a mask: `+` instead of `×` — its share (depth × shape) is added to what the other
	/// masks let through, so a row masked by distance + pulse lands where either says (owner,
	/// 2026-11-18: "add pulse instead of multiplying … glow effects on stars that are blacked
	/// out by default").
	pub add: bool,
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
			ripple: 0.0,
			every: 1.0,
			offset: 0.0,
			field: None,
			add: false,
		}
	}
}

impl Sidechain {
	pub fn shape(&self, level: f32) -> f32 {
		let t = ((level - self.threshold) / self.width.max(1e-6)).clamp(0.0, 1.0);
		// curve 0: a straight ramp (a bar's worth of growth at one speed); otherwise a smooth
		// step, raised to the curve
		if self.curve <= 0.0 {
			return if self.invert { 1.0 - t } else { t };
		}
		let t = t * t * (3.0 - 2.0 * t);
		let t = if self.curve != 1.0 { t.powf(self.curve) } else { t };
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
		base: BaseJson,
		#[serde(default)]
		chains: Vec<Sidechain>,
	},
}

impl<'de> Deserialize<'de> for Param {
	fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		Ok(match ParamJson::deserialize(d)? {
			ParamJson::Number(base) => Param::new(base),
			ParamJson::Full { base, chains } => {
				let (base, grid) = base.split();
				Param { base, chains, grid }
			}
		})
	}
}

impl Param {
	pub fn new(base: f32) -> Param {
		Param { base, chains: vec![], grid: None }
	}

	/// The base at a cell: the painted value if the base is a grid and the cell is painted,
	/// else `base`.
	pub fn base_at(&self, i: usize, j: usize) -> f32 {
		self.grid.as_ref().and_then(|g| g.at(i, j)).unwrap_or(self.base)
	}

	/// Whether the param says anything at this cell: always, unless its base is a grid
	/// that leaves the cell unpainted — a row then leaves the cell as it was.
	pub fn painted(&self, i: usize, j: usize) -> bool {
		self.grid.as_ref().map_or(true, |g| g.at(i, j).is_some())
	}

	pub fn value(&self, signals: &Signals) -> f32 {
		self.value_gated(signals, 1.0)
	}

	/// `value`, with every sidechain scaled by `gate` except those on a fixed pattern (noise,
	/// distance, bearing, around, position) or a state (hotspot, unlock): how a row is read,
	/// so it moves only as its unlock comes in. (A field's own parameters pass the gate on to
	/// their sidechains, so a pattern's motion waits too.)
	pub fn value_gated(&self, signals: &Signals, gate: f32) -> f32 {
		self.value_at(signals, gate, 0)
	}

	/// `value_gated` at `depth` fields deep (a field's parameters are params; three deep is enough).
	pub fn value_at(&self, signals: &Signals, gate: f32, depth: u8) -> f32 {
		self.base_at(signals.i, signals.j) + self.parts(signals, gate, depth).iter().sum::<f32>()
	}

	/// What each sidechain adds right now (0 for one that is off): the probe's breakdown.
	pub fn parts(&self, signals: &Signals, gate: f32, depth: u8) -> Vec<f32> {
		self.chains
			.iter()
			.map(|chain| {
				if !chain.on {
					return 0.0;
				}
				let scale = if Signals::fixed(&chain.source) { 1.0 } else { gate };
				scale * chain.depth * chain.shape(signals.level(chain, gate, depth))
			})
			.collect()
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
	/// The way round the layer's rectangle at the cell: 0 on its top edge, 0.75 the right,
	/// 0.5 the bottom, 0.25 the left (and inside) — × 360° on angle sends lines clockwise.
	pub around: f32,
	/// For `hotspot:<id>` (1 while that hotspot of the scene is showing: unlocked, not
	/// spent) and `unlock:<name>` (its reveal, 0..1); and the noise map, for fields.
	pub model: &'a Model,
	pub tags: &'a [(String, u8)],
	/// The cell: its index, and where it is in cells right of and up from the centre dot.
	pub i: usize,
	pub j: usize,
	pub di: f32,
	pub dj: f32,
	/// The scene's time (seconds × 0.6), for `clock`.
	pub time: f32,
	/// Where the music is in the track, in bars (fractional), for things that step with it.
	pub bars: f32,
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
			"around" => self.around,
			"bar.phase" => self.whole,
			"beat.phase" => self.quarter,
			"clock" => self.time.rem_euclid(1.0),
			"bass" => self.bass,
			"mid" => self.mid,
			"treble" => self.treble,
			source => {
				if let Some(id) = source.strip_prefix("hotspot:") {
					let tag = self.tags.iter().find(|(hotspot, _)| hotspot == id).map(|(_, tag)| *tag);
					if tag.map_or(false, |tag| self.model.hint_visible(tag)) { 1.0 } else { 0.0 }
				} else if let Some(name) = source.strip_prefix("unlock:") {
					unlock_named(name).map_or(0.0, |unlock| self.model.reveal(unlock))
				} else {
					// a band's level, or `id.pitch` / `id.chroma`
					self.bands.get(source).copied().unwrap_or(0.0)
				}
			}
		}
	}

	/// Is this source a fixed pattern or a state of the game rather than motion — so it
	/// acts whatever is unlocked? (A position field's parameters gate their own motion.)
	pub fn fixed(source: &str) -> bool {
		matches!(source, "noise" | "distance" | "bearing" | "around" | "position") || source.starts_with("hotspot:") || source.starts_with("unlock:")
	}

	/// A sidechain's or mask's level: its field at the cell if it is one, else its source,
	/// delayed by its `ripple` × the cell's distance.
	pub fn level(&self, chain: &Sidechain, gate: f32, depth: u8) -> f32 {
		match &chain.field {
			Some(field) if chain.source == "position" => {
				if depth >= 3 {
					0.0
				} else {
					field.eval(self, gate, depth + 1)
				}
			}
			_ => self.get_timed(&chain.source, chain.ripple * self.distance, chain.every, chain.offset),
		}
	}

	/// `get_delayed`, with the beat and bar sources on a cycle of `every` beats (bars) from
	/// the downbeat, `offset` of them in: the hit falls away over one beat (bar) as always
	/// and stays down until the next; the phases run 0..1 over the whole cycle.
	pub fn get_timed(&self, source: &str, shift: f32, every: f32, offset: f32) -> f32 {
		if (every - 1.0).abs() < 1e-6 && offset.abs() < 1e-6 {
			return self.get_delayed(source, shift);
		}
		let n = every.max(1e-3);
		let cycle = |pos: f32| ((pos - offset).rem_euclid(n) / n - shift).rem_euclid(1.0);
		match source {
			"beat" => hit(cycle(self.bars * 4.0) * n.max(1.0), 2.5),
			"bar" => hit(cycle(self.bars) * n.max(1.0), 1.5),
			"beat.phase" => cycle(self.bars * 4.0),
			"bar.phase" => cycle(self.bars),
			_ => self.get_delayed(source, shift),
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
			"bar.phase" => whole,
			"beat.phase" => (self.quarter - shift).rem_euclid(1.0),
			"clock" => (self.time - shift).rem_euclid(1.0),
			_ => self.get(source),
		}
	}
}

// ---- the mechanics ----

/// What the editor needs to know about the kinds and their params. Types: `param` (a base
/// and sidechains), `number`, `bool`, `angle`, `select` (with `options`), `sub`.
pub const CATALOGUE: &str = r##"{
  "sources": ["span", "beat", "bar", "pulse", "bar.phase", "beat.phase", "clock", "noise", "distance", "bearing", "around", "transition", "bass", "mid", "treble"],
  "fields": [],
  "elsewhere": [],
  "sourceDocs": {
    "span": "0..1 across the layer's own bars (the scene's if it has none): where we are in it",
    "beat": "hits on every beat and falls away before the next",
    "bar": "hits on the downbeat and falls away over the bar",
    "pulse": "beat and bar together: beat hits, downbeat hardest",
    "bar.phase": "0 at the downbeat running to 1 at the next: the bar as a ramp — drive a field's phase with it",
    "beat.phase": "0 at each beat running to 1 at the next",
    "clock": "0 to 1 every time unit (≈ 1.7 s), over and over, music or no music — for things that just turn",
    "position": "a field: a function of where the cell is, from the library (ripple, spiral, orbit, …), with parameters you can sidechain — a pattern that can breathe, spin or travel",
    "noise": "a fixed random value per cell, 0..1 — a pattern, not motion",
    "transition": "0..1 along a via passage out of the scene",
    "bass": "the old coarse bands: 0-2.7 kHz",
    "mid": "2.7-11 kHz",
    "treble": "11-22 kHz",
    "distance": "how far the cell is from the centre dot: 0 there, 1 at a corner (a pattern; shape it with threshold and width)",
    "bearing": "which way the cell is from the centre dot: 0 straight up, round clockwise to 1 (a pattern)",
    "around": "the way round the layer's rectangle at the cell: 0 on its top edge, 0.75 the right, 0.5 the bottom, 0.25 the left and inside — × 360 on angle sends lines clockwise round it (a pattern)",
    "hotspot": "1 while that hotspot of the scene is showing (the player has what it needs and hasn't spent it), else 0 — as a mask, the layer is a hint; invert it for the opposite",
    "unlock": "how far that unlock has come in, 0..1 — as a mask, the row shows only once it's had; invert it to fade out instead"
  },
  "rows": [],
  "kinds": {
    "cells": {
      "rows": true,
      "doc": "Every cell in the region: a dot, with a line out of it and a ring round it. Twelve rows, one per unlock; this layer sets, adds to or multiplies the rows it has, over what the layers before it left (nothing: a dark dot). A new layer is just brightness 1. Mask a row by distance, bearing or around (where it lands), by a hotspot (a hint) or an unlock (once it's had); sidechain it from the music to move it, which comes in with its unlock."
    },
    "snake": {
      "doc": "One line through every cell of the region, row by row (or column by column), as a snake — and a snake of its own through each separate piece of the region (every other column, each blob of a map), all running at once. retreat: it pulls back cell by cell from its start, leaving a dot at each. arrive: from the dots, each dot in turn extends (and turns) a line to the next until the line is whole. Where it is comes from `progress`: the layer's own bars by default, so put a retreat and an arrive on consecutive bars. Not a cells layer: rows don't reach it.",
      "params": {
        "mode": {"type": "select", "options": ["retreat", "arrive"], "default": "retreat"},
        "axis": {"type": "select", "options": ["rows", "cols"], "default": "rows", "doc": "the lines run along rows (left-right) or columns (up-down)"},
        "alternate": {"type": "bool", "default": true, "doc": "every other line runs the opposite way, so it's one continuous snake"},
        "reverse": {"type": "bool", "default": false, "doc": "run the order from the other end"},
        "heads": {"type": "number", "default": 1, "step": 1, "doc": "how many snakes at once (the order cut into equal runs)"},
        "tail": {"type": "bool", "default": false, "doc": "the last dot of a run extends a segment too, carrying straight on (past the edge of the grid if it must): every dot owns one, so bars are as long as their dots and a column folds whole"},
        "spin": {"type": "number", "default": 0, "step": 15, "doc": "degrees the moving end turns about its dot as it grows or shrinks"},
        "progress": {"type": "param", "default": {"base": 0, "chains": [{"source": "span", "depth": 1, "threshold": 0, "width": 1, "curve": 1, "invert": false, "on": true}]}, "step": 0.05, "doc": "0 untouched .. 1 done, for the mode"},
        "brightness": {"type": "param", "default": 0.9, "step": 0.05, "doc": "the line's; sidechains act once glow is unlocked"},
        "dots": {"type": "param", "default": 0.45, "step": 0.05, "doc": "the dots it leaves / starts from, likewise"},
        "color": {"type": "param", "default": 0, "step": 0.1}
      }
    },
    "bars": {
      "doc": "A grid of dots where every dot owns one segment. Extend: every other dot along the bottom grows a line up to the next dot, which grows to the next, and so on off the top; in the other columns a line comes down from the top dot the same way. Retreat: in reverse, into the dots they came from. Then extend from the other end of every line, and retreat into that — the columns bounce. Four parts of `per` beats each — a bar each by default, a cycle of 4 bars. With the angle unlock, every segment turns a quarter turn on its own dot over the first beat of each retreat, each its own way (a pinwheel: even rows end up pointing right, odd rows left), and the retreat runs as rows while it turns; the rows grow back from the other side, turn again the same way, and retreat as columns — every line comes from the other side each time round, so the pattern repeats every two cycles. Lines gates the segments, length their motion (without it every segment is a full pitch), angle the turn. Not a cells layer: rows don't reach it otherwise.",
      "params": {
        "per": {"type": "number", "default": 4, "step": 1, "doc": "beats each part lasts, on the track's beats: 4 is a bar each — extend, retreat, extend, retreat, a cycle in 4 bars; the turn takes one beat of a retreat. 0: `progress` drives it instead"},
        "progress": {"type": "param", "default": {"base": 0, "chains": [{"source": "span", "depth": 1, "threshold": 0, "width": 1, "curve": 1, "invert": false, "on": true}]}, "step": 0.05, "doc": "with `per` 0: 0..1 through two cycles (eight parts)"},
        "brightness": {"type": "param", "default": 0.9, "step": 0.05, "doc": "the lines'; sidechains act once glow is unlocked"},
        "dots": {"type": "param", "default": 0.6, "step": 0.05, "doc": "the grid's dots, 0 to leave them to the layers below"},
        "color": {"type": "param", "default": 0, "step": 0.1}
      }
    },
    "text": {
      "doc": "Words, in the owner's lattice font: one letter per box between four dots, drawn as strokes in the piece's line weight. The text fills the region's rectangle from its top-left box, wrapping at the rectangle's width (a box short of the dots: eleven dots hold ten letters a line); a line break in the text starts a new line. Capitals and small letters are different glyphs; a case not drawn borrows the other. Not a cells layer: rows don't reach it — brightness and color are its own.",
      "params": {
        "text": {"type": "string", "multiline": true, "default": "words", "doc": "what it says"},
        "hotspot": {"type": "hotspot", "default": "", "doc": "or say a hotspot's name instead, while that hotspot shows, placed wherever this layer is — an exit's name (name 1 … name 4 is one of the room's random words); a hotspot that sends you to a stage says that stage's random word, centred across this layer, once you have been to the stage"},
        "align": {"type": "select", "options": ["left", "center", "right"], "default": "left"},
        "lead": {"type": "number", "default": 1, "step": 1, "doc": "rows per line"},
        "width": {"type": "number", "default": 1.5, "step": 0.1, "doc": "how much of a box the letter grid spans across (the letters use the middle of it)"},
        "height": {"type": "number", "default": 1.5, "step": 0.1, "doc": "and down"},
        "center": {"type": "bool", "default": true, "doc": "each letter shifted to the middle of its box"},
        "reveal": {"type": "param", "default": 1, "step": 0.05, "doc": "0..1: how much of the text is there, letter by letter — sidechain it to span for a typewriter"},
        "brightness": {"type": "param", "default": 0.9, "step": 0.05, "doc": "sidechains act once glow is unlocked"},
        "color": {"type": "param", "default": 0, "step": 0.1}
      }
    },
    "hole": {
      "doc": "A hole: in its region, nothing the layers before it drew shows — their lines and rings go (scenery too: bars, snakes, letters), the dots stay unless told otherwise; a layer after it draws there as usual. Tied to a hotspot it is that hotspot's cells, while the hotspot shows: lines passing over a door vanish there. Not a cells layer.",
      "params": {
        "hotspot": {"type": "hotspot", "default": "", "doc": "cut that hotspot's cells instead of this layer's region, while it shows"},
        "dots": {"type": "bool", "default": true, "doc": "leave the dots"},
        "direction": {"type": "select", "options": ["auto", "horizontal", "vertical", "any"], "default": "auto", "doc": "which lines it cuts: the ones running along it — auto reads the shape: a wide hole cuts horizontal lines, a tall one vertical, a square one everything; a line turning through dims as it comes to lie along the hole"}
      }
    },
    "orbits": {
      "doc": "A point orbits each cell and a line runs out to it; a second, invisible point orbits further out the other way, and `pull` drags the tip of the line toward it. Not a cells layer.",
      "params": {
        "near": {"type": "number", "default": 0.45, "step": 0.05, "doc": "orbit radius, cells"},
        "far": {"type": "number", "default": 0.9, "step": 0.05, "doc": "target radius, cells"},
        "near_rate": {"type": "number", "default": 0.8, "step": 0.1},
        "far_rate": {"type": "number", "default": 1.3, "step": 0.1},
        "vary": {"type": "number", "default": 0.4, "step": 0.1, "doc": "how much the rates vary per cell"},
        "pull": {"type": "param", "default": 0, "step": 0.1, "doc": "0 at the orbit .. 1 at the target"},
        "brightness": {"type": "param", "default": 0.6, "step": 0.05},
        "color": {"type": "param", "default": 0, "step": 0.1}
      }
    },
    "walk": {
      "modifier": true,
      "doc": "A grid of regions lit one per tick of a subdivision, in reading order: this 16th happens here, the next one there. Adds to the brightness and length of the cells under it.",
      "params": {
        "cols": {"type": "number", "default": 4, "step": 1},
        "rows": {"type": "number", "default": 4, "step": 1},
        "sub": {"type": "select", "options": ["whole", "half", "quarter", "eighth", "sixteenth"], "default": "sixteenth"},
        "brightness": {"type": "number", "default": 0.25, "step": 0.05},
        "length": {"type": "number", "default": 0.8, "step": 0.1, "doc": "cells, falling over the tick"}
      }
    }
  }
}"##;

/// `CATALOGUE` with the rows filled in from `ROWS`, for the editor.
pub fn catalogue() -> String {
	let rows: Vec<Value> = ROWS
		.iter()
		.map(|row| {
			serde_json::json!({
				"name": row.name,
				"unlock": row.unlock.name(),
				"start": row.start,
				"default": row.default,
				"gate": match row.gate { Gate::Sidechains => Value::from("sidechains"), Gate::Row(v) => Value::from(v) },
				"step": row.step,
				"doc": row.doc,
			})
		})
		.collect();
	// The hand-written scenes (for_against.rs, …): ids a door can lead to that aren't in the
	// album, so the editor's graph knows a door to one of them isn't a door to nowhere.
	let elsewhere: Vec<&str> = super::for_against::SCENES
		.iter()
		.chain(super::terra_firmament::SCENES.iter())
		.map(|scene| scene.id)
		.collect();
	CATALOGUE
		.replace("\"rows\": [],", &format!("\"rows\": {},", serde_json::to_string(&rows).unwrap()))
		.replace("\"fields\": [],", &format!("\"fields\": {},", fields::fields_json()))
		.replace("\"elsewhere\": [],", &format!("\"elsewhere\": {},", serde_json::to_string(&elsewhere).unwrap()))
}

// ---- the rows of a cell ----

/// How a cells layer changes a row of the cell under it.
#[derive(Deserialize, Clone, Copy, PartialEq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Op {
	#[default]
	Set,
	Add,
	Multiply,
}

/// One row of a cells layer: `op` the cell's value with `param` (a base — one number, or
/// painted cell by cell — plus sidechains), by as much as its `masks` let land — each a source shaped like a sidechain, 1 letting
/// the whole change through, 0 none (× `depth`: how much of the mask applies). Masks on
/// distance, bearing and around are the owner's masks; on `hotspot:<id>` and
/// `unlock:<name>` they are how a layer shows only while a hotspot does (a hint) or once
/// something is unlocked. The angle rows (angle, rotation) can also `spin`, in degrees per
/// time unit — every other cell the other way if `alternate`.
#[derive(Clone, Debug)]
pub struct Row {
	pub op: Op,
	pub param: Param,
	pub masks: Vec<Sidechain>,
	pub spin: f32,
	pub alternate: bool,
}

/// A row in the file is a bare number (set to it) or `{op, base, chains, masks, spin, alternate}`.
#[derive(Deserialize)]
#[serde(untagged)]
enum RowJson {
	Number(f32),
	Full {
		#[serde(default)]
		op: Op,
		#[serde(default)]
		base: BaseJson,
		#[serde(default)]
		chains: Vec<Sidechain>,
		#[serde(default)]
		masks: Vec<Sidechain>,
		#[serde(default)]
		spin: f32,
		#[serde(default)]
		alternate: bool,
	},
}

impl<'de> Deserialize<'de> for Row {
	fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		Ok(match RowJson::deserialize(d)? {
			RowJson::Number(base) => Row {
				op: Op::Set,
				param: Param::new(base),
				masks: vec![],
				spin: 0.0,
				alternate: false,
			},
			RowJson::Full {
				op,
				base,
				chains,
				masks,
				spin,
				alternate,
			} => {
				let (base, grid) = base.split();
				Row {
					op,
					param: Param { base, chains, grid },
					masks,
					spin,
					alternate,
				}
			}
		})
	}
}

impl Row {
	/// How much of this row's change lands: the product of its `×` masks, each `1 − depth ×
	/// (1 − shape)`, plus the share of each `+` mask, `depth × shape` (so with only `+` masks
	/// nothing lands until one says so); a moving one scaled by `gate` like a sidechain.
	pub fn amount(&self, signals: &Signals, gate: f32) -> f32 {
		let parts = self.mask_parts(signals, gate);
		let (mut product, mut sum, mut any_mul, mut any_add) = (1.0, 0.0, false, false);
		for (mask, part) in self.masks.iter().zip(parts) {
			if !mask.on {
				continue;
			}
			if mask.add {
				sum += part;
				any_add = true;
			} else {
				product *= part;
				any_mul = true;
			}
		}
		let through = if any_mul { product } else if any_add { 0.0 } else { 1.0 };
		(through + sum).clamp(0.0, 1.0)
	}

	/// What each mask does right now — a `×` mask what it lets through (1 when off), a `+`
	/// mask what it adds (0 when off): the probe's breakdown.
	pub fn mask_parts(&self, signals: &Signals, gate: f32) -> Vec<f32> {
		self.masks
			.iter()
			.map(|mask| {
				if !mask.on {
					return if mask.add { 0.0 } else { 1.0 };
				}
				let scale = if Signals::fixed(&mask.source) { 1.0 } else { gate };
				let shaped = mask.shape(signals.level(mask, gate, 0));
				if mask.add {
					scale * mask.depth * shaped
				} else {
					1.0 - scale * mask.depth * (1.0 - shaped)
				}
			})
			.collect()
	}
}

/// What a row's unlock is (owner: "an unlock basically just means either a row or a
/// sidechain option, so glow is really just the brightness sidechain option").
/// `Sidechains`: the row is free from the start — dots at any brightness, lines pointing
/// anywhere — and the unlock is its motion: the sidechains and masks on the music, the
/// spin. `Row(v)`: the unlock is the row itself; it sits at `v` until then and eases to
/// the layers' value as the unlock comes in.
#[derive(Clone, Copy)]
pub enum Gate {
	Sidechains,
	Row(f32),
}

/// One row of a cell, the thing one unlock unlocks.
pub struct RowSpec {
	pub name: &'static str,
	pub unlock: Unlock,
	/// The cell's value under no layer at all.
	pub start: f32,
	/// The base of a row newly added in the editor.
	pub default: f32,
	pub gate: Gate,
	/// The editor's step for its base.
	pub step: f32,
	pub doc: &'static str,
}

/// The rows of a cell, in order: one per unlock. The drawing order of the cell is the
/// dot, the line out of it (× `lines`), the ring round it (× `rings`).
/// The rows the renderer reaches into by index (the hole kind); keep with `ROWS`.
const ROW_BRIGHTNESS: usize = 0;
const ROW_LINES: usize = 2;
const ROW_ANGLE: usize = 4;
const ROW_RINGS: usize = 6;

pub const ROWS: [RowSpec; Unlock::COUNT] = [
	RowSpec {
		name: "brightness",
		unlock: Unlock::Glow,
		start: 0.0,
		default: 1.0,
		gate: Gate::Sidechains,
		step: 0.05,
		doc: "0 dark .. 1 full, of the dot, its line and its ring. Free from the start; glow is its sidechain option — it follows the music once glow is unlocked.",
	},
	RowSpec {
		name: "dots",
		unlock: Unlock::Dots,
		start: 1.0,
		default: 1.0,
		gate: Gate::Sidechains,
		step: 0.1,
		doc: "0 no dot .. 1 the dot (× brightness) — seen where the cell has no line, since a line covers its dot. Free from the start, so a layer or a painted base can take the dots off some cells; dots is its sidechain option — the dot follows the music once dots is unlocked.",
	},
	RowSpec {
		name: "lines",
		unlock: Unlock::Lines,
		start: 0.0,
		default: 1.0,
		gate: Gate::Row(0.0),
		step: 0.1,
		doc: "0 no line .. 1 the line (× its length). Nothing until lines are unlocked; then it grows out of the dot. Mask it by distance or bearing for lines only here.",
	},
	RowSpec {
		name: "length",
		unlock: Unlock::Length,
		start: 1.0,
		default: 1.0,
		gate: Gate::Row(1.0),
		step: 0.1,
		doc: "in cells; 1 — to the next dot — until length is unlocked Negative: the other way out of the dot.",
	},
	RowSpec {
		name: "angle",
		unlock: Unlock::Angle,
		start: 0.0,
		default: 0.0,
		gate: Gate::Sidechains,
		step: 15.0,
		doc: "degrees anticlockwise from pointing right: 0 along, 90 across. Free from the start; angle is its sidechain option — it turns (and spins) once angle is unlocked. Patterns: noise × 360 scatters; bearing × −360 + 90 is radial, + 180 tangent; around × 360 runs round the rectangle.",
	},
	RowSpec {
		name: "bend",
		unlock: Unlock::Bend,
		start: 0.0,
		default: 0.0,
		gate: Gate::Row(0.0),
		step: 0.1,
		doc: "0 straight .. 1 a half circle turning left; negative turns right. Straight until bend is unlocked.",
	},
	RowSpec {
		name: "rings",
		unlock: Unlock::Rings,
		start: 0.0,
		default: 1.0,
		gate: Gate::Row(0.0),
		step: 0.1,
		doc: "0 no ring .. 1 the ring (× its radius). Nothing until rings are unlocked; then it grows out of the dot.",
	},
	RowSpec {
		name: "radius",
		unlock: Unlock::Size,
		start: RING,
		default: RING,
		gate: Gate::Row(RING),
		step: 0.05,
		doc: "in cells; 0.4 until size is unlocked",
	},
	RowSpec {
		name: "fill",
		unlock: Unlock::Fill,
		start: 0.0,
		default: 0.0,
		gate: Gate::Row(0.0),
		step: 0.1,
		doc: "0 an outline .. 1 a disc; an outline until fill is unlocked",
	},
	RowSpec {
		name: "morph",
		unlock: Unlock::Morph,
		start: 0.0,
		default: 0.0,
		gate: Gate::Row(0.0),
		step: 0.1,
		doc: "0 a circle .. 1 a square; a circle until morph is unlocked",
	},
	RowSpec {
		name: "flatness",
		unlock: Unlock::Flatness,
		start: 0.0,
		default: 0.0,
		gate: Gate::Row(0.0),
		step: 0.1,
		doc: "0 round .. 1 squashed flat across its rotation; round until flatness is unlocked",
	},
	RowSpec {
		name: "rotation",
		unlock: Unlock::Rotation,
		start: 0.0,
		default: 0.0,
		gate: Gate::Row(0.0),
		step: 15.0,
		doc: "degrees the ring is turned (and spins); 0 until rotation is unlocked",
	},
	RowSpec {
		name: "color",
		unlock: Unlock::Color,
		start: 0.0,
		default: 0.0,
		gate: Gate::Row(0.0),
		step: 0.1,
		doc: "0 grey .. 1 the color; grey until color is unlocked",
	},
];

/// A cells layer: the rows it changes.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Cells {
	brightness: Option<Row>,
	dots: Option<Row>,
	lines: Option<Row>,
	length: Option<Row>,
	angle: Option<Row>,
	bend: Option<Row>,
	rings: Option<Row>,
	radius: Option<Row>,
	fill: Option<Row>,
	morph: Option<Row>,
	flatness: Option<Row>,
	rotation: Option<Row>,
	color: Option<Row>,
}

impl Cells {
	/// The row at `k` in `ROWS`.
	fn row(&self, k: usize) -> Option<&Row> {
		[
			&self.brightness,
			&self.dots,
			&self.lines,
			&self.length,
			&self.angle,
			&self.bend,
			&self.rings,
			&self.radius,
			&self.fill,
			&self.morph,
			&self.flatness,
			&self.rotation,
			&self.color,
		][k]
			.as_ref()
	}
}

// ---- the other kinds ----

/// Words in the lattice font (`font.rs`, the owner's, drawn connect-the-dots): a letter per
/// box between four dots, laid out at install (`TextLayout`).
#[derive(Deserialize)]
#[serde(default)]
struct Text {
	text: String,
	/// A hotspot's id: say its name instead of `text` — "name N" the room's random word
	/// (`names.rs`) — while that hotspot shows. The exits' names, placed where the owner wants.
	hotspot: String,
	align: String,
	lead: usize,
	width: f32,
	height: f32,
	center: bool,
	reveal: Param,
	brightness: Param,
	color: Param,
}

impl Default for Text {
	fn default() -> Self {
		Text {
			text: "words".into(),
			hotspot: String::new(),
			align: "left".into(),
			lead: 1,
			width: font::WIDTH,
			height: font::HEIGHT,
			center: font::CENTER,
			reveal: Param::new(1.0),
			brightness: Param::new(0.9),
			color: Param::new(0.0),
		}
	}
}

/// A hole: in its region nothing the layers before it drew shows — their lines and rings go,
/// the dots stay unless `dots` is off. Tied to a hotspot, it is the hotspot's cells, while
/// the hotspot shows (owner, 2026-11-19: "i want the lines to be invisible as they pass over
/// those hotspots").
#[derive(Deserialize)]
#[serde(default)]
struct Hole {
	hotspot: String,
	dots: bool,
	/// Which lines it cuts: `auto` reads the shape — a wide hole the horizontal lines, a tall
	/// one the vertical, a square one all — or `horizontal`, `vertical`, `any` (owner: "there
	/// shouldn't be vertical holes in the horizontal bars and vice versa").
	direction: String,
}

impl Default for Hole {
	fn default() -> Self {
		Hole {
			hotspot: String::new(),
			dots: true,
			direction: "auto".into(),
		}
	}
}

/// Columns of segments growing dot to dot from alternating ends and retreating, bouncing
/// between the ends; with the angle unlock every segment pivots a quarter turn on its own
/// dot as a retreat begins, so the retreat runs as rows (owner, 2026-11-18: "every other dot
/// on the bottom grows a line upward, and when it touches the next dot, that dot grows to
/// the next dot etc. … all of the line segments … rotate in such a way that they form
/// alternating lines horizontally, and then the lines retreat in the reverse animation";
/// 2026-11-20: the bounce, the turn over one beat with the unlock, cycles of 4 bars).
#[derive(Deserialize)]
#[serde(default)]
struct Bars {
	/// Beats a part lasts (extend, retreat, extend, retreat), on the track's beats; 0 for
	/// `progress`.
	per: f32,
	progress: Param,
	brightness: Param,
	dots: Param,
	color: Param,
}

impl Default for Bars {
	fn default() -> Self {
		Bars {
			per: 4.0,
			progress: Param {
				base: 0.0,
				chains: vec![Sidechain {
					source: "span".into(),
					..Default::default()
				}],
				grid: None,
			},
			brightness: Param::new(0.9),
			dots: Param::new(0.6),
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
	/// The last cell of a run owns a segment too, continuing the line.
	tail: bool,
	/// Degrees.
	spin: f32,
	progress: Param,
	brightness: Param,
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
			tail: false,
			spin: 0.0,
			progress: Param {
				base: 0.0,
				chains: vec![Sidechain {
					source: "span".into(),
					..Default::default()
				}],
				grid: None,
			},
			brightness: Param::new(0.9),
			dots: Param::new(0.45),
			color: Param::new(0.0),
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
	brightness: Param,
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
			brightness: Param::new(0.6),
			color: Param::new(0.0),
		}
	}
}

#[derive(Deserialize)]
#[serde(default)]
struct Walk {
	cols: usize,
	rows: usize,
	sub: String,
	brightness: f32,
	length: f32,
}

impl Default for Walk {
	fn default() -> Self {
		Walk {
			cols: 4,
			rows: 4,
			sub: "sixteenth".into(),
			brightness: 0.25,
			length: 0.8,
		}
	}
}

enum Kind {
	Cells(Cells),
	Snake(Snake),
	Bars(Bars),
	Text(Text),
	Orbits(Orbits),
	Walk(Walk),
	Hole(Hole),
}

fn parse_kind(kind: &str, params: &Value) -> Result<Kind, String> {
	let params = match params {
		Value::Object(_) => params.clone(),
		_ => Value::Object(Default::default()),
	};
	let bad = |e: serde_json::Error| format!("{kind}: {e}");
	Ok(match kind {
		"cells" => Kind::Cells(serde_json::from_value(params).map_err(bad)?),
		"snake" => Kind::Snake(serde_json::from_value(params).map_err(bad)?),
		"bars" => Kind::Bars(serde_json::from_value(params).map_err(bad)?),
		"hole" => Kind::Hole(serde_json::from_value(params).map_err(bad)?),
		"text" => Kind::Text(serde_json::from_value(params).map_err(bad)?),
		"orbits" => Kind::Orbits(serde_json::from_value(params).map_err(bad)?),
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

/// The `around` signal: the way round `area` at the cell, 0..1 clockwise from the top edge
/// (0 top, 0.75 right, 0.5 bottom, 0.25 left — and inside), so × 360° on angle it is the
/// direction that goes round the rectangle clockwise.
fn around_level(area: Area, i: usize, j: usize) -> f32 {
	if j + 1 == area.j1 && i + 1 < area.i1 {
		0.0
	} else if i + 1 == area.i1 && j > area.j0 {
		0.75
	} else if j == area.j0 && i > area.i0 {
		0.5
	} else {
		0.25
	}
}

// ---- built scenes ----

struct LayerRt {
	layer: Layer,
	kind: Kind,
	/// Index of the stage it belongs to, if one.
	stage: Option<usize>,
	/// A snake layer's region cut into its pieces.
	snake: Option<SnakePaths>,
	/// A text layer's letters, placed.
	text: Option<TextLayout>,
	/// A text layer saying a hotspot's name, or a hole where a hotspot is: that hotspot's
	/// tag — there while it shows.
	hotspot: Option<u8>,
	/// A text layer saying a stage hotspot's name: that stage — the name shows once the
	/// player has been there (`model.stage_visited`).
	to_stage: Option<usize>,
	/// A hole's direction: the way (radians) of the lines it cuts; None, all of them.
	direction: Option<f32>,
}

/// A text layer laid out: which box (by its bottom-left dot) holds which glyph. The region's
/// rectangle of dots has one box fewer across and down than dots; lines start at the top-left
/// box and go down `lead` rows; words wrap at the width, a word too long is cut; a line break
/// in the text is one.
struct TextLayout {
	cells: HashMap<(usize, usize), TextCell>,
	/// Letters placed (spaces don't count), for `reveal`.
	count: usize,
}

struct TextCell {
	/// Its place in the text, for `reveal`.
	index: usize,
	strokes: &'static [[u8; 4]],
	/// Grid points to move it across, for `center` — and half a box when a line is
	/// centred across an odd number of spare boxes.
	shift: f32,
}

impl TextLayout {
	/// `centred`: the lines sit in the middle of the width whatever `align` says, to the
	/// half box (a stage's name, owner 2026-11-21: "centered horizontally in the available space").
	fn new(region: &Region, text: &Text, content: &str, centred: bool) -> Self {
		let mut out = TextLayout { cells: HashMap::new(), count: 0 };
		let b = region.bounds();
		let (cols, rows) = ((b.i1 - b.i0).saturating_sub(1), (b.j1 - b.j0).saturating_sub(1));
		if cols == 0 || rows == 0 {
			return out;
		}
		let mut lines: Vec<Vec<char>> = vec![];
		for para in content.split('\n') {
			let mut line: Vec<char> = vec![];
			for word in para.split(' ').filter(|w| !w.is_empty()) {
				let w: Vec<char> = word.chars().collect();
				if !line.is_empty() && line.len() + 1 + w.len() > cols && w.len() <= cols {
					lines.push(std::mem::take(&mut line));
				}
				if !line.is_empty() {
					line.push(' ');
				}
				for &c in &w {
					if line.len() >= cols {
						lines.push(std::mem::take(&mut line));
					}
					line.push(c);
				}
			}
			lines.push(line);
		}
		let lead = text.lead.max(1);
		for (n, line) in lines.iter().enumerate() {
			let row = n * lead;
			if row >= rows {
				break;
			}
			let j = b.j1 - 2 - row;
			let spare = cols.saturating_sub(line.len());
			let offset = match text.align.as_str() {
				_ if centred => spare / 2,
				"center" => spare / 2,
				"right" => spare,
				_ => 0,
			};
			// Half a box to the right, in grid points: the glyph grid's `COLS` points span
			// `width` boxes (see `glyph()`).
			let half = if centred && spare % 2 == 1 { font::COLS / (2.0 * text.width.max(0.1)) } else { 0.0 };
			for (k, &c) in line.iter().enumerate() {
				let Some(strokes) = font::glyph(c) else { continue };
				let shift = half + if text.center { font::shift(strokes) } else { 0.0 };
				out.cells.insert((b.i0 + offset + k, j), TextCell { index: out.count, strokes, shift });
				out.count += 1;
			}
		}
		out
	}
}

/// One letter: its strokes, in grid points, scaled into the box whose bottom-left dot is at
/// (x, y) — `width`/`height` of the box about its middle — drawn with `stroke()`, so in the
/// piece's weight, and morphing like any mark.
/// What a hotspot is called: a random word of the game for the stage it sends you to, if it
/// sends you to one (owner, 2026-11-21: "it's the stages that i want to have random names,
/// all of them"; a stage the scene doesn't have: its name as written); otherwise its name,
/// "name N" being one of the room's random words.
fn label_of(hotspot: &HotspotData, stages: &[String], room: usize) -> String {
	for action in &hotspot.then {
		if let ActionData::Stage(stage) = action {
			return match stages.iter().position(|s| s == stage) {
				Some(index) => names::stage(room, index).to_string(),
				None => stage.clone(),
			};
		}
	}
	names::resolve(&hotspot.name, room)
}

/// The stage a hotspot sends you to, if one the scene has.
fn stage_of(hotspot: &HotspotData, stages: &[String]) -> Option<usize> {
	hotspot.then.iter().find_map(|action| match action {
		ActionData::Stage(stage) => stages.iter().position(|s| s == stage),
		_ => None,
	})
}

fn glyph(draw: &Draw, model: &Model, x: f32, y: f32, unit: f32, cell: &TextCell, width: f32, height: f32, color: Rgb) {
	let px = |g: f32| x + (0.5 + (g / font::COLS - 0.5) * width) * unit;
	let py = |g: f32| y + ((1.0 - height) / 2.0 + g / font::ROWS * height) * unit;
	for s in cell.strokes {
		let (x1, y1) = (px(s[0] as f32 + cell.shift), py(s[1] as f32));
		let (x2, y2) = (px(s[2] as f32 + cell.shift), py(s[3] as f32));
		let (dx, dy) = (x2 - x1, y2 - y1);
		stroke(draw, model, x1, y1, dy.atan2(dx), (dx * dx + dy * dy).sqrt(), color);
	}
}

/// The region of a snake layer as separate (4-connected) pieces, each its cells in
/// serpentine order — a whole rectangle is one piece, so it threads as it always did; every
/// other column, or each blob of a map, is its own snake (owner, 2026-11-18: "each region
/// gets its own effect").
struct SnakePaths {
	pieces: Vec<Vec<(usize, usize)>>,
	/// Cell → (piece, index along it).
	at: HashMap<(usize, usize), (usize, usize)>,
}

impl SnakePaths {
	fn new(region: &Region, snake: &Snake) -> Self {
		let mut seen = HashMap::new();
		let mut pieces = vec![];
		for i0 in 0..CANVAS_WIDTH {
			for j0 in 0..CANVAS_HEIGHT {
				if !region.contains(i0, j0) || seen.contains_key(&(i0, j0)) {
					continue;
				}
				// flood the piece
				let mut cells = vec![(i0, j0)];
				seen.insert((i0, j0), ());
				let mut k = 0;
				while k < cells.len() {
					let (i, j) = cells[k];
					k += 1;
					let around = [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)];
					for (ni, nj) in around {
						if ni < CANVAS_WIDTH && nj < CANVAS_HEIGHT && region.contains(ni, nj) && !seen.contains_key(&(ni, nj)) {
							seen.insert((ni, nj), ());
							cells.push((ni, nj));
						}
					}
				}
				// serpentine order within the piece's own rectangle: lines top-down along rows
				// (or left-right along columns), every other line the other way
				let (imin, jmax) = (cells.iter().map(|c| c.0).min().unwrap(), cells.iter().map(|c| c.1).max().unwrap());
				let jmin = cells.iter().map(|c| c.1).min().unwrap();
				let (iw, jh) = (cells.iter().map(|c| c.0).max().unwrap() + 1 - imin, jmax + 1 - jmin);
				let along_rows = snake.axis != "cols";
				let key = |&(i, j): &(usize, usize)| {
					let (line, pos, len) = if along_rows { (jmax - j, i - imin, iw) } else { (i - imin, j - jmin, jh) };
					let pos = if snake.alternate && line % 2 == 1 { len - 1 - pos } else { pos };
					(line, pos)
				};
				cells.sort_by_key(key);
				if snake.reverse {
					cells.reverse();
				}
				pieces.push(cells);
			}
		}
		let at = pieces
			.iter()
			.enumerate()
			.flat_map(|(p, cells)| cells.iter().enumerate().map(move |(k, &cell)| (cell, (p, k))))
			.collect();
		SnakePaths { pieces, at }
	}
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
			// Rooms are the album's scenes in order: this one's number, for its exits' names.
			let room = scenes.len();
			let mut layers = vec![];
			for layer in scene.layers {
				let kind = parse_kind(&layer.kind, &layer.params)
					.map_err(|e| format!("{} layer {:?}: {e}", scene.id, layer.name))?;
				let stage = stage_index(&layer.stage);
				let snake = match &kind {
					Kind::Snake(snake) => Some(SnakePaths::new(&layer.region, snake)),
					_ => None,
				};
				let mut layer = layer;
				let (text, hotspot, direction, to_stage) = match &kind {
					Kind::Hole(hole) => {
						let hotspot = (!hole.hotspot.is_empty())
							.then(|| scene.hotspots.iter().position(|h| h.id == hole.hotspot))
							.flatten();
						if let Some(k) = hotspot {
							let h = &scene.hotspots[k];
							layer.region = if h.region.is_unset() {
								Region {
									rect: h.rect,
									..Default::default()
								}
							} else {
								h.region.clone()
							};
						}
						let b = layer.region.bounds();
						let (w, h) = (b.i1.saturating_sub(b.i0), b.j1.saturating_sub(b.j0));
						let direction = match hole.direction.as_str() {
							"horizontal" => Some(0.0),
							"vertical" => Some(std::f32::consts::FRAC_PI_2),
							"any" => None,
							_ if w > h => Some(0.0),
							_ if h > w => Some(std::f32::consts::FRAC_PI_2),
							_ => None,
						};
						(None, hotspot.map(|k| k as u8), direction, None)
					}
					Kind::Text(text) => {
						// Saying a hotspot's name: its tag is its place in the list. A stage
						// hotspot's name is centred, and waits for the stage to be visited.
						let hotspot = (!text.hotspot.is_empty())
							.then(|| scene.hotspots.iter().position(|h| h.id == text.hotspot))
							.flatten();
						let to_stage = hotspot.and_then(|k| stage_of(&scene.hotspots[k], &scene.stages));
						let content = match hotspot {
							Some(k) => label_of(&scene.hotspots[k], &scene.stages, room),
							None => text.text.clone(),
						};
						let layout = TextLayout::new(&layer.region, text, &content, to_stage.is_some());
						(Some(layout), hotspot.map(|k| k as u8), None, to_stage)
					}
					_ => (None, None, None, None),
				};
				layers.push(LayerRt { layer, kind, stage, snake, text, hotspot, direction, to_stage });
			}
			let layers: &'static [LayerRt] = Vec::leak(layers);
			let bars = scene.bars[0]..scene.bars[1].max(scene.bars[0] + 1);
			let loop_from = scene.loop_from.unwrap_or(bars.start).clamp(bars.start, bars.end - 1);
			let stages: Vec<&'static str> = scene.stages.iter().map(|s| leak_str(s)).collect();
			let delay = |name: &str| scene.unlock_delays.get(name).copied().unwrap_or(0.0).max(0.0);
			let delays: Vec<f32> = if scene.stages.is_empty() {
				vec![delay("")]
			} else {
				scene.stages.iter().map(|s| delay(s)).collect()
			};
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
				delays: Vec::leak(delays),
				render,
				hotspots: Vec::leak(hotspots),
			}));
		}
	}
	let count = scenes.len();
	DATA.with(|data| *data.borrow_mut() = (scenes, album.bands));
	Ok(count)
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
	let first_mark = model.marks_len();
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
		around: 0.0,
		model,
		tags: &scene.tags,
		i,
		j,
		di,
		dj,
		time,
		bars: bar,
		whole,
		quarter,
	};
	// The same signals for one layer: `span` where the bar is in its own bars, `around`
	// where the cell is on its rectangle.
	let for_layer = |rt: &LayerRt| {
		let [from, to] = rt.layer.bars.unwrap_or(scene.bars);
		let span = if to > from {
			((bar - from as f32) / (to - from) as f32).clamp(0.0, 1.0)
		} else {
			0.0
		};
		Signals {
			span,
			around: around_level(rt.layer.region.area(), i, j),
			..signals
		}
	};
	let stage = model.stage();
	let active = || {
		scene.layers.iter().filter(|rt| {
			rt.stage.map_or(true, |s| s == stage)
				&& rt.layer.bars.map_or(true, |[from, to]| from as f32 <= bar && bar < to as f32)
				&& rt.layer.region.contains(i, j)
		})
	};
	let glow = model.reveal(Unlock::Glow);

	// The walks first: what they add to the brightness and length of the cell under them.
	let (mut hot_brightness, mut hot_length) = (0.0, 0.0);
	for rt in active() {
		if let Kind::Walk(walk) = &rt.kind {
			let sub = subdivision(&walk.sub);
			let grid = Regions::Grid {
				cols: walk.cols.max(1),
				rows: walk.rows.max(1),
			};
			if !rt.layer.muted && grid.is_current(model, sub, i, j) {
				let fall = 1.0 - modifiers[sub as usize];
				hot_brightness += walk.brightness * fall;
				hot_length += walk.length * fall;
			}
		}
	}

	// The cell's rows start as nothing — a dark dot — and each cells layer over it, in
	// order, sets, adds to or multiplies the rows it has, by as much as each row's masks
	// let land. The other kinds draw as they go.
	let mut cell: [f32; ROWS.len()] = std::array::from_fn(|k| ROWS[k].start);
	let mut covered = false;
	for rt in active() {
		if rt.layer.muted {
			continue;
		}
		let signals = for_layer(rt);
		match &rt.kind {
			Kind::Cells(cells) => {
				covered = true;
				for (k, spec) in ROWS.iter().enumerate() {
					let Some(row) = cells.row(k) else {
						continue;
					};
					// a painted base leaves the cells it doesn't paint alone
					if !row.param.painted(i, j) {
						continue;
					}
					// Where the unlock is the row's sidechain option, its motion — the sidechains
					// and masks on the music, the spin — comes in with the unlock; where it is
					// the row, the row is eased from its resting value once the cell is folded.
					let motion = match spec.gate {
						Gate::Sidechains => model.reveal(spec.unlock),
						Gate::Row(_) => 1.0,
					};
					let dir = if row.alternate && (i + j) % 2 == 1 { -1.0 } else { 1.0 };
					let value = row.param.value_gated(&signals, motion) + row.spin * time * dir * motion;
					let applied = match row.op {
						Op::Set => value,
						Op::Add => cell[k] + value,
						Op::Multiply => cell[k] * value,
					};
					let amount = row.amount(&signals, motion);
					cell[k] += (applied - cell[k]) * amount;
					if (i, j) == model.probe && !model.previewing() {
						model.probed.borrow_mut().push(crate::model::Probed {
							layer: rt.layer.id.clone(),
							row: spec.name,
							op: match row.op {
								Op::Set => "set",
								Op::Add => "add",
								Op::Multiply => "multiply",
							},
							value,
							amount,
							chains: row.param.parts(&signals, motion, 0),
							masks: row.mask_parts(&signals, motion),
						});
					}
				}
			}
			Kind::Snake(snake) => {
				// One polyline through the cell centres of each piece of the region in serpentine
				// order (`SnakePaths`); cell k owns the segment from its centre to the next cell's.
				let Some(paths) = &rt.snake else { continue };
				let Some(&(piece, k)) = paths.at.get(&(i, j)) else { continue };
				let cells = &paths.pieces[piece];
				let n = cells.len();
				let cell_at = |k: usize| cells[k];
				let heads = snake.heads.clamp(1, n);
				let run = (n + heads - 1) / heads;
				let (head, c) = (k / run, k % run);
				let last = ((head + 1) * run).min(n) - 1;
				let run_len = last + 1 - head * run;
				let front = snake.progress.value(&signals).clamp(0.0, 1.0) * run_len as f32;
				let color = snake.color.value(&signals);
				let dots = snake.dots.value_gated(&signals, glow) + hot_brightness;
				dot(draw, model, x, y, paint(model, dots, color));
				if k < last || snake.tail {
					// the segment's far end: the next cell, or for the last cell with a tail, one
					// more step the way the line was going (along the axis if it is alone)
					let (qx, qy) = if k < last {
						let (ni, nj) = cell_at(k + 1);
						scale_coordinates(model, ni, nj)
					} else {
						let (di, dj) = if k > 0 && paths.at.get(&cell_at(k - 1)).map_or(false, |&(p, _)| p == piece) {
							let (pi, pj) = cell_at(k - 1);
							(i as f32 - pi as f32, j as f32 - pj as f32)
						} else if snake.axis != "cols" {
							(if snake.reverse { -1.0 } else { 1.0 }, 0.0)
						} else {
							(0.0, if snake.reverse { -1.0 } else { 1.0 })
						};
						(x + di * unit, y + dj * unit)
					};
					let arrive = snake.mode == "arrive";
					let c = c as f32;
					let fill = if arrive { front - c } else { c + 1.0 - front }.clamp(0.0, 1.0);
					if fill > 0.0 {
						// arriving grows out of this dot towards the next; retreating pulls back
						// from this dot towards the next, anchored there
						let (sx, sy, dx, dy) = if arrive { (x, y, qx - x, qy - y) } else { (qx, qy, x - qx, y - qy) };
						let angle = dy.atan2(dx) + snake.spin.to_radians() * (1.0 - fill);
						let brightness = snake.brightness.value_gated(&signals, glow) + hot_brightness;
						stroke(draw, model, sx, sy, angle, fill * unit, paint(model, brightness, color));
					}
				}
			}
			Kind::Bars(bars) => {
				// Every dot owns one segment, a dot's pitch long, pointing up in even columns
				// and down in odd ones. Four parts to a cycle, each `per` beats (a bar):
				// extend — even columns climb from the bottom dot, each dot's segment growing to
				//   the next once the line reaches it, the last one off the grid; odd columns
				//   come down from the top the same way;
				// retreat — in reverse, back into the dots they came from;
				// extend again, from the other end of every line; retreat into that.
				// So the columns bounce (owner, 2026-11-20: "the bars just bounce back and forth
				// vertically"). With the angle unlock every segment turns a quarter turn on its
				// own dot over the first beat of each retreat, each its own way — a pinwheel, the
				// way that points even rows right and odd rows left — and the retreat runs as
				// rows while it turns ("just have them all turn 90 degrees over like one beat in
				// the same direction they're doing now and start the next extend/retract
				// animation instantly"); the rows then grow back from the other side, turn again
				// the same way, and retreat as columns into the ends they first grew from. The
				// next cycle's columns come from the other end, so every line comes from the
				// other side each time round: the pattern repeats every two cycles.
				// Lines gates the segments (they grow out of their dots with it), length their
				// motion (without it every segment is a full pitch), angle the turn.
				let color = bars.color.value(&signals);
				let dots = bars.dots.value_gated(&signals, glow) + hot_brightness;
				if dots > 0.001 {
					dot(draw, model, x, y, paint(model, dots, color));
				}
				let last = CANVAS_HEIGHT - 1;
				let count = CANVAS_HEIGHT as f32; // segments along a line, the last off the grid
				let t = if bars.per > 0.0 {
					(signals.bars * 4.0 / bars.per).rem_euclid(8.0)
				} else {
					bars.progress.value(&signals).rem_euclid(1.0) * 8.0
				};
				let cycle = (t / 4.0).floor() as usize;
				let part = (t.floor() as usize) % 4;
				let f = t - t.floor();
				let lines = model.reveal(Unlock::Lines);
				let length = model.reveal(Unlock::Length);
				let turning = model.reveal(Unlock::Angle);
				let angled = turning > 0.0;
				let (col_even, row_even) = (i % 2 == 0, j % 2 == 0);
				let up = std::f32::consts::FRAC_PI_2;
				// the way this cell's segment points as a column — the other way every other
				// cycle once the segments turn, so the columns come from the other end
				let mut vertical = if col_even { up } else { -up };
				if angled && cycle == 1 {
					vertical = -vertical;
				}
				// the pinwheel: which way this segment turns, so the columns tile into rows
				let sense = if col_even == row_even { -up } else { up };
				// the quarter turn over the first beat of a retreat
				let beat = if bars.per > 0.0 { bars.per } else { 4.0 };
				let quick = (f * beat).min(1.0) * turning;
				// the angle drawn; the way whose root the line is counted from; growing or not
				let (angle, pointing, grow) = match (part, angled) {
					(0, _) => (vertical, vertical, true),
					(1, false) => (vertical, vertical, false),
					(1, true) => (vertical + sense * quick, vertical + sense, false),
					(2, false) => (-vertical, -vertical, true),
					(2, true) => (vertical - sense, vertical - sense, true),
					(_, false) => (-vertical, -vertical, false),
					(_, true) => (vertical - sense + sense * quick, vertical, false),
				};
				// segments between the line's root and this dot
				let (cx, cy) = (pointing.cos(), pointing.sin());
				let n = if cy.abs() > cx.abs() {
					if cy > 0.0 { j } else { last - j }
				} else if cx > 0.0 {
					i
				} else {
					last - i
				} as f32;
				let moving = if grow { f * count - n } else { (1.0 - f) * count - n }.clamp(0.0, 1.0);
				// without length every segment is a full pitch; the motion eases in with it
				let fill = (1.0 + (moving - 1.0) * length) * lines;
				if fill <= 0.001 {
					continue;
				}
				let brightness = bars.brightness.value_gated(&signals, glow) + hot_brightness;
				stroke(draw, model, x, y, angle, fill * unit, paint(model, brightness, color));
			}
			Kind::Text(text) => {
				// The box whose bottom-left dot this is holds a letter: its strokes, in grid
				// points, scaled into the box (`width`/`height` of it, about its middle).
				let Some(layout) = &rt.text else { continue };
				// A hotspot's name shows while the hotspot does (needs met, not spent, its stage)
				// — a stage's name once the player has been to the stage.
				if rt.hotspot.is_some_and(|tag| !model.hint_visible(tag)) {
					continue;
				}
				if rt.to_stage.is_some_and(|stage| !model.stage_visited(stage)) {
					continue;
				}
				let Some(cell) = layout.cells.get(&(i, j)) else { continue };
				let shown = (text.reveal.value(&signals).clamp(0.0, 1.0) * layout.count as f32).ceil() as usize;
				if cell.index >= shown {
					continue;
				}
				let brightness = text.brightness.value_gated(&signals, glow) + hot_brightness;
				if brightness <= 0.001 {
					continue;
				}
				let color = paint(model, brightness, text.color.value(&signals));
				glyph(draw, model, x, y, unit, cell, text.width, text.height, color);
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
				let brightness = orbits.brightness.value_gated(&signals, glow) + hot_brightness;
				let color = orbits.color.value(&signals);
				dot(draw, model, x, y, paint(model, 0.9, 0.0));
				dot(draw, model, x1, y1, paint(model, 0.5, 0.0));
				mark(
					draw,
					model,
					x,
					y,
					(ey - y).atan2(ex - x),
					0.0,
					((ex - x).powi(2) + (ey - y).powi(2)).sqrt(),
					brightness,
					color,
				);
			}
			Kind::Walk(_) => {}
			Kind::Hole(hole) => {
				if rt.hotspot.is_some_and(|tag| !model.hint_visible(tag)) {
					continue;
				}
				// What the layers before it drew here goes: the scenery marks recorded so far
				// this cell, and the cells rows' line and ring (a later cells layer can set them
				// again).
				model.cut_marks(first_mark, hole.dots, rt.direction);
				match rt.direction {
					None => {
						cell[ROW_LINES] = 0.0;
						cell[ROW_RINGS] = 0.0;
					}
					// the cells' line too, by how far it lies along the hole
					Some(dir) => cell[ROW_LINES] *= 1.0 - (cell[ROW_ANGLE].to_radians() - dir).cos().powi(2),
				}
				if !hole.dots {
					cell[ROW_BRIGHTNESS] = 0.0;
				}
			}
		}
	}
	if !covered {
		return;
	}

	// A row that is itself an unlock rests at its value until then, then eases to the cell's.
	for (k, spec) in ROWS.iter().enumerate() {
		if let Gate::Row(before) = spec.gate {
			cell[k] = before + (cell[k] - before) * model.reveal(spec.unlock);
		}
	}
	let [brightness, dots, lines, length, angle, bend, rings, radius, fill, morph, flatness, rotation, color] = cell;
	// A black mark is nothing on a black ground — except that, drawn after the other kinds'
	// marks, it would cover their dots (owner: "i'd like there to be dots on all the cells").
	if brightness + hot_brightness <= 0.001 {
		return;
	}
	let dot = tint((brightness * dots.clamp(0.0, 1.0) + hot_brightness).max(0.0), color);
	let color = tint(brightness + hot_brightness, color);
	// The dot — or the line out of it: a line of no length draws as the dot, at the dots
	// row's share of the brightness; a line covers its dot, so dots only show where there
	// is no line.
	// A negative length is the line the other way out of the dot (so a dot can draw its
	// pitch backward — the bars bounce as two layers).
	let line = (length + hot_length) * lines * unit;
	let (angle, line) = if line < 0.0 { (angle + 180.0, -line) } else { (angle, line) };
	if line > 0.001 || brightness * dots + hot_brightness > 0.001 {
		shape(
			draw,
			model,
			x,
			y,
			Shape::Line {
				angle: angle.to_radians(),
				length: line,
				bend,
			},
			if line > 0.001 { color } else { dot },
		);
	}
	if rings > 0.0 {
		shape(
			draw,
			model,
			x,
			y,
			Shape::Ring {
				radius: radius * rings * unit,
				weight: WEIGHT,
				fill,
				square: morph,
				flat: flatness,
				angle: rotation.to_radians(),
			},
			color,
		);
	}
}
