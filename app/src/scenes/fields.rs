//! Position as a source: **fields** — functions of where the cell is, each with parameters
//! that are themselves params (sidechainable), so a pattern can breathe, spin, travel.
//!
//! A sidechain or mask whose `source` is `position` carries a `Field` — `{fn, params}` —
//! and its level is the field at the cell, 0..1, read through the chain's threshold /
//! width / curve / invert as usual. Every field sees the cell relative to a **centre**
//! (`x`, `y`: cells right/up of the grid's centre dot) and most take a **turn** (turns,
//! clockwise, 0 up — the direction the pattern faces; sidechain it to spin the pattern)
//! and a **phase** (cycles; sidechain it from `bar.phase`, `beat.phase`, `span` or
//! `clock` to animate). Radial ones take a **shape**: −1 a diamond, 0 round, 1 a square.
//!
//! `FIELDS` is the library: name, family, a motivation, and the parameters with their
//! defaults; `fields_json()` hands it to the editor. Variety over perfection: each is a
//! few lines, and they compose — several masks multiply, several sidechains add.

use {
	super::data::{Param, Signals},
	crate::model::{CANVAS_HEIGHT, CANVAS_WIDTH},
	nannou::{
		noise::{NoiseFn, Perlin},
		prelude::*,
	},
	serde::Deserialize,
	serde_json::{json, Value},
	std::collections::HashMap,
};

/// A field in the file: `{"fn": "ripple", "params": {"period": 2, "phase": {"base": 0, "chains": [...]}}}`.
/// Parameters not given are at their defaults.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Field {
	#[serde(rename = "fn")]
	pub name: String,
	#[serde(default)]
	pub params: HashMap<String, Param>,
}

pub struct ParamSpec {
	pub name: &'static str,
	pub default: f32,
	pub step: f32,
	pub doc: &'static str,
}

pub struct FieldSpec {
	pub name: &'static str,
	pub family: &'static str,
	pub doc: &'static str,
	pub params: &'static [ParamSpec],
}

const fn p(name: &'static str, default: f32, step: f32, doc: &'static str) -> ParamSpec {
	ParamSpec {
		name,
		default,
		step,
		doc,
	}
}

const X: ParamSpec = p("x", 0.0, 0.5, "centre, cells right of the centre dot");
const Y: ParamSpec = p("y", 0.0, 0.5, "centre, cells up from it");
const TURN: ParamSpec = p("turn", 0.0, 0.05, "which way it faces: turns clockwise from up (sidechain to spin it)");
const SHAPE: ParamSpec = p("shape", 0.0, 0.1, "−1 a diamond, 0 round, 1 a square");
const PHASE: ParamSpec = p("phase", 0.0, 0.05, "where in its cycle (sidechain from bar.phase, beat.phase, span or clock to run)");
const EDGE: ParamSpec = p("edge", 0.5, 0.1, "softness of the boundary, cells");
const SHARP: ParamSpec = p("sharp", 0.0, 0.1, "0 a smooth wave .. 1 hard bands");

/// Half the grid, centre dot to the middle of an edge, in cells.
const HALF: f32 = (CANVAS_WIDTH as f32 - 1.0) / 2.0;

pub const FIELDS: &[FieldSpec] = &[
	// ---- radial: functions of how far ----
	FieldSpec {
		name: "distance",
		family: "radial",
		doc: "how far from the centre, 0 there .. 1 at `reach`. The plain gradient; with shape, a square or diamond one.",
		params: &[p("reach", 5.0, 0.5, "cells at which it reaches 1"), SHAPE, X, Y, TURN],
	},
	FieldSpec {
		name: "lamp",
		family: "radial",
		doc: "a light at the centre falling off like a lamp: 1 there, half at `knee`, dimmer with distance. Pulse the knee with the kick.",
		params: &[p("knee", 2.0, 0.25, "cells at which it is half"), p("power", 2.0, 0.5, "how fast it falls past the knee"), SHAPE, X, Y, TURN],
	},
	FieldSpec {
		name: "disc",
		family: "radial",
		doc: "a spotlight: 1 inside `radius`, 0 outside. Sidechain the radius to the bass for a disc that breathes.",
		params: &[p("radius", 2.5, 0.25, "cells"), EDGE, SHAPE, X, Y, TURN],
	},
	FieldSpec {
		name: "ring",
		family: "radial",
		doc: "a halo: a band `width` wide at `radius`. Run the radius from 0 to the corner for a ring that leaves the centre.",
		params: &[p("radius", 3.0, 0.25, "cells"), p("width", 1.0, 0.25, "cells"), EDGE, SHAPE, X, Y, TURN],
	},
	FieldSpec {
		name: "ripple",
		family: "radial",
		doc: "rings every `period` cells, travelling outward as `phase` runs: waves from a stone dropped in the middle, one wavelength per bar.",
		params: &[p("period", 2.0, 0.25, "cells between crests"), PHASE, SHARP, SHAPE, X, Y, TURN],
	},
	FieldSpec {
		name: "tunnel",
		family: "radial",
		doc: "ripple with rings packed toward the centre and spread out at the edge — running the phase is flying down a tunnel.",
		params: &[p("density", 1.5, 0.25, "rings per doubling of distance"), PHASE, SHARP, SHAPE, X, Y, TURN],
	},
	FieldSpec {
		name: "shockwave",
		family: "radial",
		doc: "one ring that leaves the centre at phase 0, reaches `reach` at phase 1 and fades as it goes: a hit radiating out, once per bar.",
		params: &[p("reach", 7.0, 0.5, "cells it travels"), p("width", 1.5, 0.25, "cells"), PHASE, SHAPE, X, Y, TURN],
	},
	// ---- angular: functions of which way ----
	FieldSpec {
		name: "bearing",
		family: "angular",
		doc: "which way from the centre, 0 at `turn` round clockwise to 1: a sweep. Spin the turn for a radar.",
		params: &[TURN, X, Y],
	},
	FieldSpec {
		name: "petals",
		family: "angular",
		doc: "`count` lobes round the centre, a flower; sharp makes them pie slices. Run the phase to spin, sidechain the count to bloom.",
		params: &[p("count", 6.0, 1.0, "lobes"), PHASE, SHARP, TURN, X, Y],
	},
	FieldSpec {
		name: "beam",
		family: "angular",
		doc: "a wedge of light pointing `turn`, `width` turns wide, with a `trail` fading behind it: a searchlight, a radar sweep with afterglow.",
		params: &[p("width", 0.1, 0.02, "turns"), p("trail", 0.0, 0.05, "turns of afterglow behind it"), p("edge", 0.03, 0.01, "softness, turns"), TURN, X, Y],
	},
	FieldSpec {
		name: "sweep",
		family: "angular",
		doc: "a pie that fills clockwise from `turn` as the phase runs 0 to 1: a timer, the bar as a clock face.",
		params: &[PHASE, p("edge", 0.03, 0.01, "softness, turns"), TURN, X, Y],
	},
	FieldSpec {
		name: "mirror",
		family: "angular",
		doc: "0 toward `turn`, 1 away from it, the same on both sides: a bilateral gradient, front to back.",
		params: &[TURN, X, Y],
	},
	FieldSpec {
		name: "kaleido",
		family: "angular",
		doc: "the bearing folded into `count` mirrored wedges: a snowflake's symmetry for anything you put through it.",
		params: &[p("count", 6.0, 1.0, "mirrored wedges"), TURN, X, Y],
	},
	FieldSpec {
		name: "spokes",
		family: "angular",
		doc: "`count` thin lines out from the centre, `width` cells thick all the way: a wheel's spokes, an asterisk.",
		params: &[p("count", 8.0, 1.0, "spokes"), p("width", 0.5, 0.1, "cells"), EDGE, TURN, X, Y],
	},
	// ---- spiral: how far and which way together ----
	FieldSpec {
		name: "spiral",
		family: "spiral",
		doc: "`arms` arms winding out, one turn per `pitch` cells: a galaxy. Run the phase and it flows out (or in, with a negative depth).",
		params: &[p("arms", 2.0, 1.0, "arms"), p("pitch", 3.0, 0.5, "cells per turn"), PHASE, SHARP, TURN, X, Y],
	},
	FieldSpec {
		name: "vortex",
		family: "spiral",
		doc: "the bearing gradient twisted by distance — `twist` turns by `reach`: a whirlpool.",
		params: &[p("twist", 0.5, 0.1, "turns of twist at reach"), p("reach", 5.0, 0.5, "cells"), PHASE, TURN, X, Y],
	},
	FieldSpec {
		name: "rose",
		family: "spiral",
		doc: "the curve r = cos(k·θ), `petals` of them, drawn `width` thick: a rhodonea. Odd counts give that many petals, even ones twice as many.",
		params: &[p("petals", 3.0, 1.0, "k"), p("size", 3.5, 0.25, "cells, tip to centre"), p("width", 0.8, 0.1, "cells"), TURN, X, Y],
	},
	FieldSpec {
		name: "star",
		family: "spiral",
		doc: "a filled star with `points`, its radius swinging by `spike` between tips and notches. Spike 0 is a disc; sidechain it to the snare.",
		params: &[p("points", 5.0, 1.0, "points"), p("size", 3.5, 0.25, "cells to a tip"), p("spike", 0.5, 0.1, "0 round .. 1 all spike"), EDGE, TURN, X, Y],
	},
	FieldSpec {
		name: "polygon",
		family: "spiral",
		doc: "a filled regular polygon with `sides`, `size` from centre to a side. Fractional sides make lopsided ones on the way.",
		params: &[p("sides", 3.0, 1.0, "sides (3 and up)"), p("size", 3.0, 0.25, "cells, centre to a side"), EDGE, TURN, X, Y],
	},
	FieldSpec {
		name: "dartboard",
		family: "spiral",
		doc: "rings every `rings` cells crossed with `sectors` slices, alternating: a checkerboard in polar coordinates.",
		params: &[p("rings", 1.5, 0.25, "cells per ring"), p("sectors", 8.0, 1.0, "slices"), PHASE, TURN, X, Y],
	},
	FieldSpec {
		name: "web",
		family: "spiral",
		doc: "`count` spokes and a ring every `period` cells, both `width` thick: a spider's web.",
		params: &[p("count", 8.0, 1.0, "spokes"), p("period", 1.5, 0.25, "cells between rings"), p("width", 0.4, 0.1, "cells"), EDGE, TURN, X, Y],
	},
	// ---- linear: functions of where along a direction ----
	FieldSpec {
		name: "gradient",
		family: "linear",
		doc: "0 at one side, 1 at the other, along `turn`, over `span` cells: a tilt. Spin the turn for a gradient that goes round.",
		params: &[p("span", 10.0, 1.0, "cells from 0 to 1"), p("offset", 0.0, 0.5, "cells the middle is shifted along turn"), TURN, X, Y],
	},
	FieldSpec {
		name: "curtain",
		family: "linear",
		doc: "1 past `offset` along `turn`, 0 before: half the grid. Run the offset from −6 to 6 (sidechain span, depth 12, base −6) for a curtain or a tide.",
		params: &[p("offset", 0.0, 0.5, "cells along turn where it starts"), EDGE, TURN, X, Y],
	},
	FieldSpec {
		name: "stripes",
		family: "linear",
		doc: "bands across `turn` every `period` cells, marching as the phase runs: a barber's pole, rain on a window.",
		params: &[p("period", 2.0, 0.25, "cells between bands"), PHASE, SHARP, TURN, X, Y],
	},
	FieldSpec {
		name: "waves",
		family: "linear",
		doc: "stripes bent into waves: swinging `amp` cells side to side every `wavelength` cells across. Water.",
		params: &[p("period", 2.0, 0.25, "cells between bands"), p("amp", 1.0, 0.25, "cells of swing"), p("wavelength", 5.0, 0.5, "cells per swing"), PHASE, SHARP, TURN, X, Y],
	},
	FieldSpec {
		name: "scanline",
		family: "linear",
		doc: "one line `width` cells thick crossing the grid along `turn` as the phase runs 0 to 1: a scan, the bar line sweeping across.",
		params: &[p("width", 1.0, 0.25, "cells"), PHASE, p("edge", 0.3, 0.1, "softness, cells"), TURN, X, Y],
	},
	FieldSpec {
		name: "lattice",
		family: "linear",
		doc: "stripes both ways: a grid of lines every `period` cells, sliding as the phase runs.",
		params: &[p("period", 2.0, 0.25, "cells"), PHASE, SHARP, TURN, X, Y],
	},
	FieldSpec {
		name: "checker",
		family: "linear",
		doc: "a checkerboard of `period`-cell squares. Run the phase to make it crawl; turn it 0.125 for diamonds.",
		params: &[p("period", 2.0, 0.25, "cells per square"), PHASE, TURN, X, Y],
	},
	FieldSpec {
		name: "plus",
		family: "linear",
		doc: "a cross through the centre, `width` cells thick: crosshairs, a plus; at turn 0.125 an x.",
		params: &[p("width", 1.0, 0.25, "cells"), EDGE, TURN, X, Y],
	},
	FieldSpec {
		name: "saddle",
		family: "linear",
		doc: "waves on the product of the two axes: hyperbolas, dense toward the corners, still along the axes. Interference you can turn.",
		params: &[p("scale", 6.0, 0.5, "cells² per wave"), PHASE, SHARP, TURN, X, Y],
	},
	// ---- pairs: two centres ----
	FieldSpec {
		name: "interference",
		family: "pairs",
		doc: "ripples from two centres `gap` apart, multiplied: moiré, the pattern two sources make together. The binary.",
		params: &[p("gap", 4.0, 0.5, "cells between the centres, along turn"), p("period", 1.5, 0.25, "cells between crests"), PHASE, TURN, X, Y],
	},
	FieldSpec {
		name: "dipole",
		family: "pairs",
		doc: "distance to the nearer of two centres `gap` apart: two wells (invert it: two lamps). Sidechain the gap to pull them together.",
		params: &[p("gap", 4.0, 0.5, "cells between the centres, along turn"), p("reach", 3.0, 0.5, "cells at which it reaches 1"), TURN, X, Y],
	},
	FieldSpec {
		name: "nearest",
		family: "pairs",
		doc: "distance to the nearest of `count` scattered points: territories, a voronoi. The phase turns the whole scatter about the centre.",
		params: &[p("count", 4.0, 1.0, "points"), p("reach", 3.0, 0.5, "cells at which it reaches 1"), PHASE, X, Y],
	},
	// ---- bodies: something moving ----
	FieldSpec {
		name: "orbit",
		family: "bodies",
		doc: "a disc `size` across going round the centre at `radius`, once per phase cycle: a satellite, a clock hand's tip.",
		params: &[p("radius", 3.0, 0.25, "cells from the centre"), p("size", 1.5, 0.25, "cells across"), PHASE, EDGE, TURN, X, Y],
	},
	FieldSpec {
		name: "comet",
		family: "bodies",
		doc: "orbit with a tail: the disc and `trail` turns of fading glow behind it on its ring.",
		params: &[p("radius", 3.0, 0.25, "cells from the centre"), p("size", 1.5, 0.25, "cells across"), p("trail", 0.25, 0.05, "turns of tail"), PHASE, EDGE, TURN, X, Y],
	},
	FieldSpec {
		name: "pendulum",
		family: "bodies",
		doc: "a disc swinging `swing` cells either side of the centre along `turn`, one swing per phase cycle.",
		params: &[p("swing", 3.0, 0.25, "cells each way"), p("size", 1.5, 0.25, "cells across"), PHASE, EDGE, TURN, X, Y],
	},
	FieldSpec {
		name: "lissajous",
		family: "bodies",
		doc: "a disc tracing a Lissajous figure — `a` swings across to `b` swings up per phase cycle — in a `width` × `height` box.",
		params: &[p("a", 3.0, 1.0, "swings across per cycle"), p("b", 2.0, 1.0, "swings up per cycle"), p("width", 8.0, 0.5, "cells"), p("height", 8.0, 0.5, "cells"), p("size", 1.2, 0.25, "cells across"), PHASE, EDGE, X, Y],
	},
	FieldSpec {
		name: "rain",
		family: "bodies",
		doc: "a drop falling down every column, each column on its own time: rain. `speed` drops per phase cycle.",
		params: &[p("size", 1.0, 0.25, "cells long"), p("speed", 1.0, 0.25, "drops per cycle"), PHASE, EDGE],
	},
	// ---- scatter: per cell ----
	FieldSpec {
		name: "noise",
		family: "scatter",
		doc: "a random value per cell from a `seed` — a different pattern for every whole number it crosses. Sidechain the seed to the kick (depth 3, threshold 0.5, width 0.01) and the pattern flashes to another on each hit; or set `every` for a fresh pattern every so many beats, locked to the music. `grain` 2 makes 2 × 2 blocks.",
		params: &[p("seed", 0.0, 1.0, "which pattern; a new one each whole number (sidechain it)"), p("every", 0.0, 0.25, "beats between fresh patterns, with the music; 0 never"), p("grain", 1.0, 1.0, "cells per block of the pattern"), X, Y],
	},
	FieldSpec {
		name: "twinkle",
		family: "scatter",
		doc: "each cell lights for `fraction` of every phase cycle at its own random moment: fireflies, a crowd of shutters.",
		params: &[p("fraction", 0.3, 0.05, "of the cycle each cell is lit"), PHASE, p("soft", 0.1, 0.05, "fade in and out, in cycles")],
	},
	FieldSpec {
		name: "reading",
		family: "scatter",
		doc: "cells in reading order — left to right, top to bottom — up to the phase: a typewriter reveal.",
		params: &[PHASE, p("soft", 0.05, 0.02, "cells of fade at the front, as a fraction of the grid")],
	},
	FieldSpec {
		name: "flow",
		family: "scatter",
		doc: "smooth noise over the grid, `scale` cells per blob, drifting as the phase runs: clouds, weather. Sidechain the phase from clock.",
		params: &[p("scale", 4.0, 0.5, "cells per blob"), PHASE, p("contrast", 1.0, 0.25, "how hard the blobs are"), X, Y],
	},
];

pub fn spec(name: &str) -> Option<&'static FieldSpec> {
	FIELDS.iter().find(|f| f.name == name)
}

/// The library for the editor's catalogue.
pub fn fields_json() -> Value {
	Value::Array(
		FIELDS
			.iter()
			.map(|f| {
				json!({
					"name": f.name,
					"family": f.family,
					"doc": f.doc,
					"params": f.params.iter().map(|q| json!({"name": q.name, "default": q.default, "step": q.step, "doc": q.doc})).collect::<Vec<_>>(),
				})
			})
			.collect(),
	)
}

thread_local! {
	static PERLIN: Perlin = Perlin::new();
}

fn frac(x: f32) -> f32 {
	x.rem_euclid(1.0)
}

fn smooth(t: f32) -> f32 {
	let t = t.clamp(0.0, 1.0);
	t * t * (3.0 - 2.0 * t)
}

/// 1 inside, 0 outside, where `d` is how far outside the boundary (negative inside),
/// softened over `edge`.
fn inside(d: f32, edge: f32) -> f32 {
	smooth(0.5 - d / edge.max(1e-3))
}

/// A wave of phase `t` (cycles), 0..1, pushed toward hard bands by `sharp`.
fn wave(t: f32, sharp: f32) -> f32 {
	let v = 0.5 + 0.5 * (TAU * t).cos();
	((v - 0.5) * (1.0 + 24.0 * sharp * sharp) + 0.5).clamp(0.0, 1.0)
}

/// A random 0..1 for three integers, the same every time.
fn hash3(a: i64, b: i64, c: i64) -> f32 {
	let mut h = (a as u32).wrapping_mul(73856093) ^ (b as u32).wrapping_mul(19349663) ^ (c as u32).wrapping_mul(83492791);
	h ^= h >> 13;
	h = h.wrapping_mul(0x5bd1e995);
	h ^= h >> 15;
	(h & 0xffff) as f32 / 65535.0
}

fn parity(n: f32) -> f32 {
	(n.floor() as i64).rem_euclid(2) as f32
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
	a + (b - a) * t
}

impl Field {
	/// The field at the cell, 0..1. `gate` scales the motion of its parameters (their
	/// sidechains on the music) like any row's; `depth` guards fields inside fields.
	pub fn eval(&self, s: &Signals, gate: f32, depth: u8) -> f32 {
		let Some(spec) = spec(&self.name) else {
			return 0.0;
		};
		let p = |name: &str| -> f32 {
			match self.params.get(name) {
				Some(param) => param.value_at(s, gate, depth),
				None => spec.params.iter().find(|q| q.name == name).map_or(0.0, |q| q.default),
			}
		};
		// the cell relative to the centre; the frame turned to face `turn`
		let (px, py) = (s.di - p("x"), s.dj - p("y"));
		let turn = p("turn");
		let (sn, cs) = (TAU * turn).sin_cos();
		let along = px * sn + py * cs;
		let across = px * cs - py * sn;
		let theta = frac(px.atan2(py) / TAU - turn);
		let shape = p("shape");
		let r = {
			let round = px.hypot(py);
			let diamond = along.abs() + across.abs();
			let square = along.abs().max(across.abs());
			if shape < 0.0 {
				lerp(round, diamond, -shape)
			} else {
				lerp(round, square, shape)
			}
		};
		let phase = p("phase");
		let edge = p("edge");
		let sharp = p("sharp");
		// a disc of `size` across at a point
		let blob = |bx: f32, by: f32, size: f32| inside((px - bx).hypot(py - by) - size / 2.0, edge);
		// how far round the centre `t` turns of direction is, in cells, at this radius
		let spokes = |count: f32, width: f32| {
			let seg = TAU / count.max(1.0);
			let a = (theta * TAU).rem_euclid(seg);
			let a = a.min(seg - a);
			inside(r * a.sin() - width / 2.0, edge)
		};
		let v = match self.name.as_str() {
			"distance" => r / p("reach").max(1e-3),
			"lamp" => 1.0 / (1.0 + (r / p("knee").max(1e-3)).powf(p("power").max(0.1))),
			"disc" => inside(r - p("radius"), edge),
			"ring" => inside((r - p("radius")).abs() - p("width") / 2.0, edge),
			"ripple" => wave(r / p("period").max(1e-3) - phase, sharp),
			"tunnel" => wave(r.max(0.25).log2() * p("density") - phase, sharp),
			"shockwave" => {
				let ph = frac(phase);
				let width = p("width");
				inside((r - ph * p("reach")).abs() - width / 2.0, width / 2.0) * (1.0 - ph)
			}
			"bearing" => theta,
			"petals" => wave(p("count") * theta + phase, sharp),
			"beam" => {
				let d = frac(theta + 0.5) - 0.5; // ahead (clockwise) positive, behind negative
				let core = inside(d.abs() - p("width") / 2.0, edge);
				let trail = p("trail");
				let tail = if d < 0.0 && trail > 0.0 { (1.0 + d / trail).clamp(0.0, 1.0) } else { 0.0 };
				core.max(tail)
			}
			"sweep" => inside(theta - phase, edge),
			"mirror" => (frac(theta + 0.5) - 0.5).abs() * 2.0,
			"kaleido" => (frac(p("count") * theta) - 0.5).abs() * 2.0,
			"spokes" => spokes(p("count"), p("width")),
			"spiral" => wave(p("arms") * theta + r / p("pitch").max(1e-3) - phase, sharp),
			"vortex" => frac(theta + p("twist") * r / p("reach").max(1e-3) + phase),
			"rose" => {
				let rho = p("size") * (TAU * p("petals") * theta).cos().abs();
				let width = p("width");
				inside((r - rho).abs() - width / 2.0, width / 2.0)
			}
			"star" => {
				let spike = p("spike").clamp(0.0, 1.0);
				let rho = p("size") * (1.0 + spike * (TAU * p("points") * theta).cos()) / (1.0 + spike);
				inside(r - rho, edge)
			}
			"polygon" => {
				let n = p("sides").max(3.0);
				let seg = TAU / n;
				let phi = theta * TAU;
				let m = (phi / seg).round();
				let boundary = p("size") / (phi - m * seg).cos().max(1e-3);
				inside(r - boundary, edge)
			}
			"dartboard" => parity((r / p("rings").max(1e-3)).floor() + (p("sectors") * theta + phase).floor()),
			"web" => {
				let period = p("period").max(1e-3);
				let dr = (r / period - (r / period).round()).abs() * period;
				inside(dr - p("width") / 2.0, edge).max(spokes(p("count"), p("width")))
			}
			"gradient" => 0.5 + (along - p("offset")) / p("span").max(1e-3),
			"curtain" => inside(p("offset") - along, edge),
			"stripes" => wave(along / p("period").max(1e-3) - phase, sharp),
			"waves" => wave((along + p("amp") * (TAU * across / p("wavelength").max(1e-3)).sin()) / p("period").max(1e-3) - phase, sharp),
			"scanline" => {
				let pos = -(HALF + 0.5) + (2.0 * HALF + 1.0) * frac(phase);
				inside((along - pos).abs() - p("width") / 2.0, edge)
			}
			"lattice" => {
				let period = p("period").max(1e-3);
				wave(along / period - phase, sharp).max(wave(across / period - phase, sharp))
			}
			"checker" => {
				let period = p("period").max(1e-3);
				parity((along / period + phase).floor() + (across / period + phase).floor())
			}
			"plus" => {
				let width = p("width");
				inside(along.abs() - width / 2.0, edge).max(inside(across.abs() - width / 2.0, edge))
			}
			"saddle" => wave(along * across / p("scale").max(1e-3) + phase, sharp),
			"interference" | "dipole" => {
				let (gx, gy) = (sn * p("gap") / 2.0, cs * p("gap") / 2.0);
				let r1 = (px - gx).hypot(py - gy);
				let r2 = (px + gx).hypot(py + gy);
				if self.name == "dipole" {
					r1.min(r2) / p("reach").max(1e-3)
				} else {
					let period = p("period").max(1e-3);
					0.5 + 0.5 * (TAU * (r1 / period - phase)).cos() * (TAU * (r2 / period - phase)).cos()
				}
			}
			"nearest" => {
				let count = p("count").max(1.0) as usize;
				let (sn, cs) = (TAU * phase).sin_cos();
				(0..count)
					.map(|k| {
						// a fixed scatter from the noise map, turned by the phase
						let nx = (s.model.noisemap.get_value(k % CANVAS_WIDTH, 1) as f32 * 2.0 - 1.0) * 4.5;
						let ny = (s.model.noisemap.get_value(1, k % CANVAS_HEIGHT) as f32 * 2.0 - 1.0) * 4.5;
						let (qx, qy) = (nx * cs - ny * sn, nx * sn + ny * cs);
						(px - qx).hypot(py - qy)
					})
					.fold(f32::INFINITY, f32::min)
					/ p("reach").max(1e-3)
			}
			"orbit" | "comet" => {
				let (radius, size) = (p("radius"), p("size"));
				let a = TAU * (phase + turn);
				let head = blob(radius * a.sin(), radius * a.cos(), size);
				if self.name == "orbit" {
					head
				} else {
					let band = inside((r - radius).abs() - size / 2.0, edge);
					let d = frac(theta - phase + 0.5) - 0.5;
					let trail = p("trail");
					let tail = if d < 0.0 && trail > 0.0 { (1.0 + d / trail).clamp(0.0, 1.0) } else { 0.0 };
					head.max(band * tail)
				}
			}
			"pendulum" => {
				let swing = p("swing") * (TAU * phase).sin();
				blob(swing * sn, swing * cs, p("size"))
			}
			"lissajous" => blob(
				p("width") / 2.0 * (TAU * p("a") * phase).sin(),
				p("height") / 2.0 * (TAU * (p("b") * phase + 0.25)).sin(),
				p("size"),
			),
			"rain" => {
				let column = s.model.noisemap.get_value(s.i, 0) as f32;
				let v = frac(phase * p("speed") + column * 3.0);
				let drop = HALF + 0.5 - (2.0 * HALF + 2.0) * v;
				inside((s.dj - drop).abs() - p("size") / 2.0, edge)
			}
			"noise" => {
				// a fresh pattern every `every` beats is motion: it waits for the row's unlock
				let every = p("every");
				let steps = if every > 0.0 && gate >= 0.5 { (s.bars * 4.0 / every).floor() } else { 0.0 };
				let seed = (p("seed") + steps).floor() as i64;
				let grain = p("grain").max(1.0);
				let (gi, gj) = (((px + HALF) / grain).floor() as i64, ((py + HALF) / grain).floor() as i64);
				hash3(gi, gj, seed)
			}
			"twinkle" => {
				let v = frac(s.noise * 7.0 - phase);
				inside(v - p("fraction"), p("soft").max(1e-3))
			}
			"reading" => {
				let order = ((CANVAS_HEIGHT - 1 - s.j) * CANVAS_WIDTH + s.i) as f32 / (CANVAS_WIDTH * CANVAS_HEIGHT - 1) as f32;
				inside(order - phase, p("soft").max(1e-3))
			}
			"flow" => {
				let scale = p("scale").max(1e-3) as f64;
				let n = PERLIN.with(|perlin| perlin.get([px as f64 / scale, py as f64 / scale, phase as f64 * 2.0])) as f32;
				0.5 + n * 1.4 * p("contrast")
			}
			_ => 0.0,
		};
		v.clamp(0.0, 1.0)
	}
}
