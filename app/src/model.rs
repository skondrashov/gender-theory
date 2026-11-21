use {
	crate::{
		audio::Audio,
		progress,
		scenes::{self, data, *},
	},
	nannou::{
		noise::{
			utils::{NoiseMap, NoiseMapBuilder, PlaneMapBuilder},
			RidgedMulti,
		},
		prelude::*,
	},
	serde_json::{json, Value},
	std::{
		cell::{Cell, RefCell},
		collections::{HashMap, HashSet},
		ops::Range,
	},
};

/// The grid: odd, so there's a centre dot (owner, 2026-11-18). Levels are designed on it and
/// it scales to the screen: a cell is the shorter side ÷ 11 (`Model::scaling`, set every
/// frame from the window; the editor's fixed square canvas ÷ 11).
pub const CANVAS_WIDTH: usize = 11;
pub const CANVAS_HEIGHT: usize = 11;
/// A cell in px before the window is known.
const SCALING: f32 = 40.0;
/// Seconds a drawn trail segment takes to fade out.
const TRAIL_LIFE: f32 = 1.5;
/// Seconds a completed outline stays lit before fading.
const TRAIL_LINGER: f32 = 1.0;
/// Seconds an unlock takes to ease in.
const REVEAL: f32 = 2.5;
/// Seconds a scene takes to morph into the next, ending as it lands: the last `MORPH`
/// seconds before the switch, or whatever is left of them if it was asked for later.
const MORPH: f64 = 1.5;
/// How fast scenes' `time` runs, in units per second (it used to count frames / 100).
const TIME_RATE: f32 = 0.6;

/// A scene change that has been asked for: scheduled once its track is decoded.
struct Pending {
	scene: &'static Scene,
	/// The stage of `scene` to land in.
	stage: usize,
	/// A new scene, so the music changes; otherwise only the stage does.
	music: bool,
	via: Option<Range<usize>>,
	/// Context time it was asked for.
	asked: f64,
	/// Context time it's scheduled for (the bar it starts on); `None` until it is.
	at: Option<f64>,
	/// Context time the new scene lands: after the via passage, if any.
	lands: Option<f64>,
	/// Don't wait for the bar (the scene menu): land in `MORPH` seconds.
	now: bool,
}

/// One unit step of the line the player is drawing, along the grid.
struct Segment {
	a: Point2,
	b: Point2,
	born: f32,
	/// On an outline being traced: stays lit.
	locked: bool,
}

pub struct Model {
	pub noise_matrix: RidgedMulti,
	pub noisemap: NoiseMap,
	pub scaling: f32,
	pub audio: Audio,
	pub debug: bool,
	/// Driven by the editor page (`?editor`): progress isn't saved, commands come in
	/// through `push_command`, state goes out through `state_json`.
	pub editor: bool,
	/// The editor asked to play a scene from one of its bars: done once the track is decoded.
	seek: Option<(String, usize)>,
	/// Cells the editor wants outlined.
	highlight: Vec<(usize, usize)>,
	/// The editor's probe: a cell whose rows the data renderer reports each frame (`probed`),
	/// so the editor can show what each row is worth there right now.
	pub probe: (usize, usize),
	pub probed: RefCell<Vec<Probed>>,
	/// The album's frequency bands, followed: id → level 0..1, and `id.pitch`, `id.chroma`
	/// (see `data::Band`).
	bands: HashMap<String, f32>,

	scene: &'static Scene,
	/// Which of the scene's stages the player is in.
	stage: usize,
	/// The stages the player has been in, a bit each, by scene id (saved): a hotspot that
	/// sends you to a stage shows its name once you have been there.
	visited: progress::Visited,
	/// (scene id, stage) as of the last update — to notice entering a stage.
	at: Option<(&'static str, usize)>,
	/// A stage that delays its unlocks, entered: (app time it landed, seconds they take to
	/// come back in). `reveal()` is held under it; hotspots that need an unlock wait for it.
	hold: Option<(f32, f32)>,
	unlocks: Unlocks,
	/// When each unlock was granted (app time), for easing it in. `-inf` = always had it.
	revealed: [f32; Unlock::COUNT],
	/// App time, as of the last update.
	time: f32,
	pending: Option<Pending>,
	/// The first click has happened (browsers gate audio on a gesture).
	started: bool,
	/// Ticks of each `Subdivision` since the current scene started.
	ticks: [usize; 5],
	phases: [f32; 5],
	pub last_whole: f64,

	/// Cell where the pointer went down, for gestures.
	press: Option<(usize, usize)>,
	/// Last lattice point of the trail being drawn.
	trail_at: Option<(usize, usize)>,
	trail: Vec<Segment>,
	/// An outline being traced: index of the hotspot in the scene, and the edges drawn so far.
	trace: Option<(usize, HashSet<Edge>)>,

	/// The scene menu (press M) is open.
	menu: bool,

	/// What the cell being rendered has drawn so far (see `scenes::Mark`).
	marks: RefCell<Vec<Mark>>,
	/// While the scene being morphed into is rendered: that scene, so that hints, ticks
	/// and the like are its own.
	preview: Cell<Option<&'static Scene>>,
	preview_stage: Cell<Option<usize>>,
}

impl Model {
	/// `start` is a scene id to begin on (`?scene=`), overriding saved progress; `unlock` is
	/// unlock names joined by `+` (`?unlock=lines+rotation`) to grant on top of it.
	pub fn new(
		debug: bool,
		start: Option<String>,
		reset: bool,
		unlock: Option<String>,
		editor: bool,
	) -> Self {
		if reset {
			progress::clear();
			crate::names::clear();
		}
		let saved = if editor { None } else { progress::load() };
		let mut unlocks = saved.as_ref().map_or(NONE, |(_, unlocks, _)| *unlocks);
		let visited = saved.as_ref().map(|(_, _, visited)| visited.clone()).unwrap_or_default();
		for name in unlock.as_deref().unwrap_or_default().split('+') {
			if let Some(unlock) = Unlock::ALL.iter().find(|unlock| unlock.name() == name) {
				unlocks = unlocks.with(*unlock);
			}
		}
		let first = *scenes::all().first().expect("no scenes");
		let scene = start
			.as_deref()
			.or(saved.as_ref().map(|(scene, _, _)| scene.as_str()))
			.and_then(scenes::find)
			.unwrap_or(first);
		for reference in scenes::dangling() {
			web_sys::console::error_1(&format!("scene reference goes nowhere: {reference}").into());
		}

		let mut model = Model {
			scaling: SCALING,
			noise_matrix: RidgedMulti::new(),
			noisemap: PlaneMapBuilder::new(&RidgedMulti::new())
				.set_size(CANVAS_WIDTH, CANVAS_HEIGHT)
				.set_x_bounds(-10.0, 10.0)
				.set_y_bounds(-10.0, 10.0)
				.build(),
			audio: Audio::new(),
			debug,
			editor,
			seek: None,
			highlight: vec![],
			probe: ((CANVAS_WIDTH - 1) / 2, (CANVAS_HEIGHT - 1) / 2),
			probed: RefCell::new(vec![]),
			bands: HashMap::new(),
			scene,
			stage: 0,
			visited,
			at: None,
			hold: None,
			unlocks,
			revealed: [f32::NEG_INFINITY; Unlock::COUNT],
			time: 0.0,
			pending: None,
			started: false,
			ticks: [0; 5],
			phases: [0.0; 5],
			last_whole: 0.0,
			press: None,
			trail_at: None,
			trail: vec![],
			trace: None,
			menu: false,
			marks: RefCell::new(vec![]),
			preview: Cell::new(None),
			preview_stage: Cell::new(None),
		};
		model.preload();
		model.note_visited();
		model
	}

	/// Has the player been in stage `stage` of the viewed scene? (A hotspot that sends you
	/// to a stage shows its name once you have.)
	pub fn stage_visited(&self, stage: usize) -> bool {
		self.visited.get(self.viewed().id).is_some_and(|bits| bits & (1u64 << stage.min(63)) != 0)
	}

	/// How far the unlocks have come back in, 0..1, in a stage that delays them — 1 when
	/// nothing holds them. The landing frame of such a stage (the morph's target) is 0.
	fn hold_in(&self) -> f32 {
		let (scene, stage) = (self.viewed(), self.stage());
		let delayed = scene.delays.get(stage).copied().unwrap_or(0.0) > 0.0;
		if delayed && (self.preview.get().is_some() || self.preview_stage.get().is_some()) {
			return 0.0;
		}
		match self.hold {
			Some((since, secs)) if secs > 0.0 => {
				let t = ((self.time - since) / secs).clamp(0.0, 1.0);
				t * t * (3.0 - 2.0 * t)
			}
			_ => 1.0,
		}
	}

	/// The stage we are in has been visited; saved when that is news. Entering it (any
	/// way: landing, the editor, a seek) starts its unlock delay, if it has one.
	fn note_visited(&mut self) {
		let here = (self.scene.id, self.stage);
		if self.at != Some(here) {
			self.at = Some(here);
			let bars = self.scene.delays.get(self.stage).copied().unwrap_or(0.0);
			self.hold = (bars > 0.0).then(|| (self.time, bars * 240.0 / self.scene.track.bpm as f32));
		}
		let bits = self.visited.entry(self.scene.id.to_string()).or_default();
		let bit = 1u64 << self.stage.min(63);
		if *bits & bit == 0 {
			*bits |= bit;
			if !self.editor {
				progress::save(self.scene.id, self.unlocks, &self.visited);
			}
		}
	}

	pub fn current_scene(&self) -> &'static Scene {
		self.scene
	}

	/// The scene being rendered: the one we're on, or the one being morphed into.
	fn viewed(&self) -> &'static Scene {
		self.preview.get().unwrap_or(self.scene)
	}

	/// The stage being rendered: the one we're in, or the one being morphed into.
	pub fn stage(&self) -> usize {
		self.preview_stage.get().unwrap_or(self.stage)
	}

	/// How many ticks of `sub` have passed since the scene started.
	pub fn ticks(&self, sub: Subdivision) -> usize {
		if self.preview.get().is_some() {
			return 0;
		}
		self.ticks[sub as usize]
	}

	/// Scenes draw through this (see `scenes::mark`, `scenes::ring`).
	pub fn record(&self, mark: Mark) {
		self.marks.borrow_mut().push(mark);
	}

	/// How many marks are recorded so far: a cell's render notes this at its start so a
	/// `hole` layer can take back what the layers before it drew there.
	pub fn marks_len(&self) -> usize {
		self.marks.borrow().len()
	}

	/// Drop the marks recorded since `from`: the dots (lines of no length) unless
	/// `keep_dots`; with no `direction` every line and ring; with one, the lines running
	/// that way (radians) — a line across it stays, one in between dims as it aligns — and
	/// rings stay.
	pub fn cut_marks(&self, from: usize, keep_dots: bool, direction: Option<f32>) {
		let mut marks = self.marks.borrow_mut();
		let kept: Vec<Mark> = marks
			.drain(from..)
			.filter_map(|mut m| match (m.shape, direction) {
				(Shape::Line { length, .. }, _) if length <= 0.0 => keep_dots.then_some(m),
				(_, None) => None,
				(Shape::Ring { .. }, Some(_)) => Some(m),
				(Shape::Line { angle, .. }, Some(dir)) => {
					let f = 1.0 - (angle - dir).cos().powi(2);
					if f < 0.01 {
						return None;
					}
					m.color = nannou::prelude::rgb(m.color.red * f, m.color.green * f, m.color.blue * f);
					Some(m)
				}
			})
			.collect();
		marks.extend(kept);
	}

	fn take_marks(&self) -> Vec<Mark> {
		std::mem::take(&mut *self.marks.borrow_mut())
	}

	pub fn has(&self, unlock: Unlock) -> bool {
		self.unlocks.has(unlock)
	}

	pub fn unlocks(&self) -> Unlocks {
		self.unlocks
	}

	/// How far an unlock has come in: 0 without it, easing up to 1 over `REVEAL` seconds
	/// from the moment it was granted, 1 for anything the player arrived with.
	pub fn reveal(&self, unlock: Unlock) -> f32 {
		if !self.has(unlock) {
			return 0.0;
		}
		let t = ((self.time - self.revealed[unlock as usize]) / REVEAL).clamp(0.0, 1.0);
		(t * t * (3.0 - 2.0 * t)).min(self.hold_in())
	}

	/// Is this frame's render the landing frame of the scene being morphed into?
	pub fn previewing(&self) -> bool {
		self.preview.get().is_some()
	}

	/// Is the viewed scene's hotspot with this `tag` showing (unlocked, not spent)?
	pub fn hint_visible(&self, tag: u8) -> bool {
		self.hotspots().any(|hotspot| hotspot.tag == tag)
	}

	/// Where the music is on the viewed scene's track, in bars (fractional). The scene
	/// being morphed into is seen at its first bar.
	pub fn bar(&self) -> f32 {
		if self.preview.get().is_some() {
			return self.viewed().bars.start as f32;
		}
		self.audio
			.bar()
			.map_or(self.scene.bars.start as f32, |bar| bar as f32)
	}

	/// The album's frequency bands as of this frame (see `data::Band`).
	pub fn bands(&self) -> &HashMap<String, f32> {
		&self.bands
	}

	/// Follow every band of the album: its level is the mean of its bins, through a
	/// one-pole follower with the band's attack and release; its pitch is the loudest bin
	/// in the range (parabolically interpolated, as the analyser's bins are in dB), on a log
	/// scale across the range, gliding with the attack, held while the band is under its
	/// floor; its chroma is that frequency folded to a pitch class.
	fn follow_bands(&mut self, dt: f32) {
		let bands = data::bands();
		if bands.is_empty() {
			self.bands.clear();
			return;
		}
		let spectrum = self.audio.spectrum();
		let bin_hz = self.audio.bin_hz();
		for band in &bands {
			let lo = ((band.lo / bin_hz).floor().max(0.0) as usize).min(spectrum.len());
			let hi = ((band.hi / bin_hz).ceil().max(0.0) as usize).clamp(lo + 1, spectrum.len().max(lo + 1));
			let bins = &spectrum[lo..hi.min(spectrum.len())];
			let level = if bins.is_empty() {
				0.0
			} else {
				bins.iter().map(|&b| b as f32).sum::<f32>() / (255.0 * bins.len() as f32)
			};
			let level = (level * band.gain).clamp(0.0, 1.0);
			let env = self.bands.entry(band.id.clone()).or_insert(0.0);
			let tau = if level > *env { band.attack } else { band.release };
			let k = if tau > 0.0 { 1.0 - (-dt / tau).exp() } else { 1.0 };
			*env += (level - *env) * k;

			if level >= band.floor && !bins.is_empty() {
				let (peak, _) = bins.iter().enumerate().max_by_key(|(_, &b)| b).unwrap();
				let offset = if peak > 0 && peak + 1 < bins.len() {
					let (a, b, c) = (bins[peak - 1] as f32, bins[peak] as f32, bins[peak + 1] as f32);
					let d = a - 2.0 * b + c;
					if d.abs() > f32::EPSILON { (0.5 * (a - c) / d).clamp(-0.5, 0.5) } else { 0.0 }
				} else {
					0.0
				};
				let hz = ((lo + peak) as f32 + offset) * bin_hz;
				let (b_lo, b_hi) = (band.lo.max(1.0), band.hi.max(band.lo.max(1.0) * 1.001));
				let pitch = ((hz.max(1.0) / b_lo).ln() / (b_hi / b_lo).ln()).clamp(0.0, 1.0);
				let chroma = ((hz.max(1.0) / 16.3516).log2() * 12.0).rem_euclid(12.0) / 12.0;
				let glide = if band.attack > 0.0 { 1.0 - (-dt / band.attack).exp() } else { 1.0 };
				let held = self.bands.entry(format!("{}.pitch", band.id)).or_insert(pitch);
				*held += (pitch - *held) * glide;
				self.bands.insert(format!("{}.chroma", band.id), chroma);
			}
		}
		self.bands.retain(|id, _| {
			let id = id.strip_suffix(".pitch").or(id.strip_suffix(".chroma")).unwrap_or(id);
			bands.iter().any(|band| band.id == id)
		});
	}

	/// The album was reinstalled (the editor): point at the same scene again.
	fn reload(&mut self) {
		let id = self.scene.id;
		let bar = self.bar().floor() as usize;
		let first = *scenes::all().first().expect("no scenes");
		let scene = scenes::find(id).unwrap_or(first);
		let bars_changed = scene.bars != self.scene.bars || scene.track.id != self.scene.track.id;
		self.pending = None;
		self.scene = scene;
		self.preload();
		if bars_changed && self.started {
			let bar = bar.clamp(scene.bars.start, scene.bars.end.saturating_sub(1));
			self.seek = Some((scene.id.to_string(), bar));
		}
	}

	/// One command from the editor page.
	fn handle(&mut self, command: Value) {
		if let Some(album) = command.get("album") {
			match data::install(&album.to_string()) {
				Ok(_) => self.reload(),
				Err(error) => web_sys::console::error_1(&error.into()),
			}
		}
		if let Some(seek) = command.get("seek") {
			let id = seek.get("scene").and_then(Value::as_str).unwrap_or("");
			let bar = seek.get("bar").and_then(Value::as_u64).unwrap_or(0) as usize;
			if let Some(scene) = scenes::find(id) {
				// Another track's scene (a song just added, a scene played from the graph):
				// nothing else would ask for its audio.
				self.audio.load(scene.track);
				self.seek = Some((id.to_string(), bar));
				self.started = true;
				self.audio.resume();
			}
		}
		if let Some(names) = command.get("unlocks").and_then(Value::as_array) {
			let mut unlocks = NONE;
			for name in names.iter().filter_map(Value::as_str) {
				if let Some(unlock) = data::unlock_named(name) {
					unlocks = unlocks.with(unlock);
				}
			}
			self.set_unlocks(unlocks);
		}
		if let Some(stage) = command.get("stage").and_then(Value::as_u64) {
			self.pending = None;
			self.stage = (stage as usize).min(self.scene.stages.len().saturating_sub(1));
		}
		if command.get("pause").is_some() {
			self.audio.suspend();
		}
		if command.get("play").is_some() {
			self.started = true;
			self.audio.resume();
		}
		if let Some(cell) = command.get("probe").and_then(Value::as_array) {
			let at = |k: usize| cell.get(k).and_then(Value::as_u64).unwrap_or(0) as usize;
			self.probe = (at(0).min(CANVAS_WIDTH - 1), at(1).min(CANVAS_HEIGHT - 1));
		}
		if let Some(cells) = command.get("highlight") {
			self.highlight = cells
				.as_array()
				.map(|cells| {
					cells
						.iter()
						.filter_map(|cell| {
							let i = cell.get(0)?.as_u64()? as usize;
							let j = cell.get(1)?.as_u64()? as usize;
							Some((i, j))
						})
						.collect()
				})
				.unwrap_or_default();
		}
	}

	/// What the editor page polls: where the music is, what's showing.
	fn state(&self) -> Value {
		let hotspots: Vec<u8> = self.hotspots().map(|hotspot| hotspot.tag).collect();
		json!({
			"scene": self.scene.id,
			"stage": self.stage,
			"stages": self.scene.stages,
			"track": self.scene.track.id,
			"bar": self.bar(),
			"phase": self.last_whole,
			"started": self.started,
			"playing": self.audio.is_playing() && !self.audio.is_suspended(),
			"loaded": self.audio.is_loaded(self.scene.track),
			"unlocks": Unlock::ALL.iter().filter(|u| self.has(**u)).map(|u| u.name()).collect::<Vec<_>>(),
			"bands": self.bands,
			"hotspots": hotspots,
			"pending": self.pending.as_ref().map(|pending| pending.scene.id),
			"probe": {
				"i": self.probe.0,
				"j": self.probe.1,
				"rows": self.probed.borrow().iter().map(|p| json!({"layer": p.layer, "row": p.row, "op": p.op, "value": p.value, "amount": p.amount, "chains": p.chains, "masks": p.masks})).collect::<Vec<_>>(),
			},
			"binHz": self.audio.bin_hz(),
		})
	}

	/// The `tag` of the unlocked hotspot covering cell (i, j), if any. Scenes draw hints with this.
	pub fn hint_at(&self, i: usize, j: usize) -> Option<u8> {
		self.hotspots()
			.find(|hotspot| hotspot.covers(i, j))
			.map(|hotspot| hotspot.tag)
	}

	/// A scene change is underway, and how far along its passage it is (0..1; 0 while it
	/// waits for the bar, or if there's no passage). Scenes intensify with this.
	pub fn transition(&self) -> Option<f32> {
		if self.preview.get().is_some() {
			return None;
		}
		let pending = self.pending.as_ref()?;
		Some(match (pending.at, pending.lands) {
			(Some(at), Some(lands)) if lands > at => {
				(((self.audio.now() - at) / (lands - at)).clamp(0.0, 1.0)) as f32
			}
			_ => 0.0,
		})
	}

	/// A scene change is underway: asked for but not landed yet. Input is blocked meanwhile.
	pub fn transitioning(&self) -> bool {
		self.pending.is_some()
	}

	/// The scene is morphing into the next one: how far along (0..1, eased), which scene,
	/// and the seconds left until it lands. The morph takes the last `MORPH` seconds before
	/// landing — or what's left of them, if the change was asked for later than that, so a
	/// late tap gets a quick morph rather than none.
	pub fn morph(&self) -> Option<(f32, &'static Scene, usize, f32)> {
		let pending = self.pending.as_ref()?;
		let lands = pending.lands?;
		let now = self.audio.now();
		let start = pending.asked.max(lands - MORPH);
		if now < start || lands <= start {
			return None;
		}
		let t = ((now - start) / (lands - start)).clamp(0.0, 1.0) as f32;
		Some((t * t * (3.0 - 2.0 * t), pending.scene, pending.stage, (lands - now).max(0.0) as f32))
	}

	/// In this stage, unlocked, and still has something to do.
	fn visible(&self, hotspot: &Hotspot) -> bool {
		hotspot.stage.map_or(true, |stage| stage == self.stage())
			&& self.unlocks.has_all(hotspot.needs)
			// in a stage that delays its unlocks, one that needs any waits for them
			&& (hotspot.needs == NONE || self.hold_in() >= 1.0)
			&& !hotspot.then.spent(self.unlocks)
	}

	/// The viewed scene's hotspots the player can see.
	fn hotspots(&self) -> impl Iterator<Item = &'static Hotspot> + '_ {
		self.viewed()
			.hotspots
			.iter()
			.filter(move |hotspot| self.visible(hotspot))
	}

	/// Start decoding every track reachable from here.
	fn preload(&mut self) {
		let mut ids: Vec<&'static str> = vec![];
		for hotspot in self.hotspots() {
			scenes::targets(&hotspot.then, &mut ids);
		}
		self.audio.load(self.scene.track);
		for id in ids {
			if let Some(scene) = scenes::find(id) {
				self.audio.load(scene.track);
			}
		}
	}

	fn act(&mut self, action: &'static Action) {
		match action {
			Action::Go(id) => self.go(id, None),
			Action::GoVia(id, via) => self.go(id, Some(via.clone())),
			Action::Unlock(unlock) => self.set_unlocks(self.unlocks.with(*unlock)),
			Action::Stage(stage) => self.go_stage(*stage),
			Action::All(list) => {
				for action in list.iter() {
					self.act(action);
				}
			}
		}
	}

	fn set_unlocks(&mut self, unlocks: Unlocks) {
		for unlock in Unlock::ALL {
			if unlocks.has(unlock) && !self.unlocks.has(unlock) {
				self.revealed[unlock as usize] = self.time;
			}
		}
		self.unlocks = unlocks;
		if !self.editor {
			progress::save(self.scene.id, self.unlocks, &self.visited);
		}
		self.preload();
	}

	/// Ask for a scene change. Ignored while one is already scheduled.
	fn go(&mut self, id: &str, via: Option<Range<usize>>) {
		if matches!(self.pending, Some(Pending { at: Some(_), .. })) {
			return;
		}
		match scenes::find(id) {
			Some(scene) => {
				self.audio.load(scene.track);
				self.pending = Some(Pending {
					scene,
					stage: 0,
					music: true,
					via,
					asked: self.audio.now(),
					at: None,
					lands: None,
					now: false,
				});
			}
			None => web_sys::console::error_1(&format!("no scene {id:?}").into()),
		}
	}

	/// Ask for another stage of this scene, at the next bar; the music goes on.
	fn go_stage(&mut self, stage: usize) {
		if matches!(self.pending, Some(Pending { at: Some(_), .. })) || stage == self.stage {
			return;
		}
		self.pending = Some(Pending {
			scene: self.scene,
			stage,
			music: false,
			via: None,
			asked: self.audio.now(),
			at: None,
			lands: None,
			now: false,
		});
	}

	/// Go somewhere right now, bar or no bar (the scene menu).
	fn jump(&mut self, scene: &'static Scene) {
		self.started = true;
		self.audio.resume();
		self.audio.load(scene.track);
		if self.audio.is_playing() {
			self.pending = Some(Pending {
				scene,
				stage: 0,
				music: true,
				via: None,
				asked: self.audio.now(),
				at: None,
				lands: None,
				now: true,
			});
		} else {
			self.enter(scene);
		}
	}

	fn enter(&mut self, scene: &'static Scene) {
		self.scene = scene;
		self.stage = 0;
		self.ticks = [0; 5];
		self.phases = [0.0; 5];
		self.trail.clear();
		self.trace = None;
		if !self.editor {
			progress::save(scene.id, self.unlocks, &self.visited);
		}
		self.preload();
	}

	/// Extend the trail from one lattice point to another along grid lines, one unit step at
	/// a time: across first, then up or down.
	fn draw_trail(&mut self, from: (usize, usize), to: (usize, usize), now: f32) {
		let mut at = from;
		while at != to {
			let next = if at.0 != to.0 {
				(if to.0 > at.0 { at.0 + 1 } else { at.0 - 1 }, at.1)
			} else {
				(at.0, if to.1 > at.1 { at.1 + 1 } else { at.1 - 1 })
			};
			self.step(at, next, now);
			at = next;
		}
	}

	/// One unit step of the trail. If it's on the outline of an `Outline` hotspot the player
	/// can see, it locks in; stepping off an outline being traced abandons the trace.
	fn step(&mut self, a: (usize, usize), b: (usize, usize), now: f32) {
		let edge = edge(a, b);
		let hotspots: &'static [Hotspot] = self.scene.hotspots;
		let on = hotspots
			.iter()
			.enumerate()
			.find(|(_, hotspot)| {
				matches!(hotspot.on, Gesture::Outline)
					&& self.visible(hotspot)
					&& hotspot.cells.perimeter().contains(&edge)
			})
			.map(|(index, _)| index);
		let tracing = self.trace.as_ref().map(|(index, _)| *index);

		let locked = match (on, tracing) {
			(Some(index), Some(current)) if current == index => {
				self.trace.as_mut().unwrap().1.insert(edge);
				true
			}
			(Some(index), _) => {
				self.abandon_trace(now);
				self.trace = Some((index, HashSet::from([edge])));
				true
			}
			(None, Some(_)) => {
				self.abandon_trace(now);
				false
			}
			(None, None) => false,
		};

		let (ax, ay) = scale_coordinates(self, a.0, a.1);
		let (bx, by) = scale_coordinates(self, b.0, b.1);
		self.trail.push(Segment {
			a: pt2(ax, ay),
			b: pt2(bx, by),
			born: now,
			locked,
		});

		if let Some((index, edges)) = &self.trace {
			let hotspot = &hotspots[*index];
			if edges.len() == hotspot.cells.perimeter().len() {
				self.trace = None;
				for segment in &mut self.trail {
					if segment.locked {
						segment.locked = false;
						segment.born = now + TRAIL_LINGER;
					}
				}
				self.act(&hotspot.then);
			}
		}
	}

	/// Let whatever was locked in fade from now.
	fn abandon_trace(&mut self, now: f32) {
		self.trace = None;
		for segment in &mut self.trail {
			if segment.locked {
				segment.locked = false;
				segment.born = now;
			}
		}
	}
}

thread_local! {
	/// Commands from the editor page, waiting for the next update.
	static COMMANDS: RefCell<Vec<String>> = RefCell::new(vec![]);
	/// The state as of the last update, for the editor page.
	static STATE: RefCell<String> = RefCell::new(String::new());
	static SPECTRUM: RefCell<Vec<u8>> = RefCell::new(vec![]);
}

pub fn push_command(command: String) {
	COMMANDS.with(|commands| commands.borrow_mut().push(command));
}

pub fn state_json() -> String {
	STATE.with(|state| state.borrow().clone())
}

pub fn spectrum_copy() -> Vec<u8> {
	SPECTRUM.with(|spectrum| spectrum.borrow().clone())
}

pub fn update(app: &App, model: &mut Model, update: Update) {
	let now = model.audio.now();
	model.time = app.time;
	if !model.editor {
		// The grid takes the shorter side of the window, whatever the window is.
		let window = app.window_rect();
		model.scaling = window.w().min(window.h()) / CANVAS_WIDTH as f32;
	}

	let commands: Vec<String> = COMMANDS.with(|commands| std::mem::take(&mut *commands.borrow_mut()));
	for command in commands {
		match serde_json::from_str::<Value>(&command) {
			Ok(command) => model.handle(command),
			Err(error) => web_sys::console::error_1(&format!("bad editor command: {error}").into()),
		}
	}

	model.audio.advance();
	model.follow_bands(update.since_last.as_secs_f32().min(0.1));
	if let Some(pending) = &model.pending {
		if pending.lands.map_or(false, |lands| now >= lands) {
			let (scene, stage, music) = (pending.scene, pending.stage, pending.music);
			model.pending = None;
			if music {
				model.enter(scene);
			}
			model.stage = stage;
		}
	}

	// The editor: play a scene from one of its bars, as soon as its track is here.
	if let Some((id, bar)) = model.seek.clone() {
		if let Some(scene) = scenes::find(&id) {
			if model.audio.is_loaded(scene.track) {
				model.seek = None;
				model.pending = None;
				let stage = if std::ptr::eq(scene, model.scene) { model.stage } else { 0 };
				model.enter(scene);
				model.stage = stage;
				let _ = model.audio.play_from(scene, bar, now + 0.05);
			}
		} else {
			model.seek = None;
		}
	}

	// Wherever the stage was set this frame (landing, the editor, a seek): we have been here.
	model.note_visited();

	if model.started {
		if !model.editor {
			model.audio.resume();
		}
		if !model.audio.is_playing() {
			// Retried every frame until the track is decoded.
			let _ = model.audio.play(model.scene, None, now);
		}
	}

	// Schedule a pending change once its track is decoded: at the next bar, or a morph
	// away for the menu. A via passage is bars of the track we're on. A stage change
	// leaves the music alone.
	if let Some(pending) = &mut model.pending {
		if pending.at.is_none()
			&& model.audio.is_playing()
			&& model.audio.is_loaded(pending.scene.track)
		{
			let at = if pending.now {
				now + MORPH
			} else {
				model.audio.next_bar()
			};
			if !pending.music {
				pending.at = Some(at);
				pending.lands = Some(at);
			} else {
				let via = pending.via.clone().map(|bars| (model.scene.track, bars));
				if let Ok(lands) = model.audio.play(pending.scene, via, at) {
					pending.at = Some(at);
					pending.lands = Some(lands);
				}
			}
		}
	}

	if let Some(whole) = model.audio.phase() {
		model.last_whole = whole;
		let phases = fold(whole as f32);
		for k in 0..5 {
			if phases[k] < model.phases[k] {
				model.ticks[k] += 1;
			}
		}
		model.phases = phases;
	}

	let time = app.time;
	model
		.trail
		.retain(|segment| segment.locked || time - segment.born < TRAIL_LIFE);

	if model.editor {
		STATE.with(|state| *state.borrow_mut() = model.state().to_string());
		SPECTRUM.with(|spectrum| *spectrum.borrow_mut() = model.audio.spectrum());
	}
}

/// What one row of one layer was worth at the probe cell this frame.
pub struct Probed {
	pub layer: String,
	pub row: &'static str,
	pub op: &'static str,
	/// The row's value (what it sets, adds or multiplies by).
	pub value: f32,
	/// How much of that landed: the product of its masks.
	pub amount: f32,
	/// What each of its sidechains added, and what each of its masks let through.
	pub chains: Vec<f32>,
	pub masks: Vec<f32>,
}

pub fn view(app: &App, model: &Model, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let modifiers = get_modifiers(model);
	model.probed.borrow_mut().clear();

	if model.audio.is_loaded(model.scene.track) {
		let time = app.time * TIME_RATE;
		let morph = model.morph();
		for i in 0..CANVAS_WIDTH {
			for j in 0..CANVAS_HEIGHT {
				(model.scene.render)(&draw, model, i, j, time, modifiers);
				let marks = model.take_marks();
				match morph {
					None => marks.iter().for_each(|mark| mark.draw(&draw)),
					Some((t, next, stage, left)) => {
						// Where the next scene will be as it lands: the top of its bar,
						// `left` seconds from now, the music as it is.
						let mut landing = [0.0; 8];
						landing[5..].copy_from_slice(&modifiers[5..]);
						model.preview.set(Some(next));
						model.preview_stage.set(Some(stage));
						(next.render)(&draw, model, i, j, time + left * TIME_RATE, landing);
						model.preview.set(None);
						model.preview_stage.set(None);
						let into = model.take_marks();
						for mark in blend(&marks, &into, t) {
							mark.draw(&draw);
						}
					}
				}
			}
		}
	}

	for segment in &model.trail {
		let alpha = if segment.locked {
			1.0
		} else {
			1.0 - (app.time - segment.born) / TRAIL_LIFE
		};
		draw.line()
			.weight(if segment.locked { 3.0 } else { 2.0 })
			.rgba(1.0, 1.0, 1.0, alpha)
			.points(segment.a, segment.b);
	}

	let window = app.window_rect();

	// The editor's selection.
	for &(i, j) in &model.highlight {
		let (x, y) = scale_coordinates(model, i, j);
		draw.rect()
			.no_fill()
			.stroke_color(rgba(1.0, 1.0, 1.0, 0.35))
			.stroke_weight(1.0)
			.x_y(x, y)
			.w_h(model.scaling as f32 - 2.0, model.scaling as f32 - 2.0);
	}

	// The track isn't here yet: a bar along the bottom filling up.
	if !model.audio.is_loaded(model.scene.track) {
		let y = window.bottom() + 40.0;
		let (left, right) = (window.left() + 40.0, window.right() - 40.0);
		draw.line()
			.weight(2.0)
			.gray(0.25)
			.points(pt2(left, y), pt2(right, y));
		if let Some(progress) = model.audio.progress(model.scene.track) {
			draw.line()
				.weight(2.0)
				.gray(if progress < 1.0 { 0.9 } else { 0.6 })
				.points(pt2(left, y), pt2(left + (right - left) * progress, y));
		}
	}

	// (A transition used to draw a white border until it landed; the morph telegraphs it
	// on its own now — owner, 2026-11-18.)

	if model.menu {
		draw_menu(&draw, model, window);
	}

	// debug overlay — enable with ?debug in the URL
	if model.debug {
		draw.text(&format!("Fps: {:?}", app.fps().round()))
			.x_y(-200.0, 220.0);

		let [whole, half, quarter, eighth, sixteenth, bass, mid, treble] = modifiers;

		draw.text(&format!(
			"1({whole:.2}) 2({half:.2}) 4({quarter:.2}) 8({eighth:.2}) 16({sixteenth:.2})"
		))
		.x_y(-200.0, 180.0);

		draw.text(&format!(
			"bass({bass:.2}) mid({mid:.2}) treble({treble:.2})"
		))
		.x_y(-200.0, 160.0);

		let fft = model
			.audio
			.fft()
			.iter()
			.map(|&value| format!("{:.2}|", value as f32 / 255.0))
			.collect::<String>();
		draw.text(&format!("FFT: {fft}")).x_y(200.0, 160.0);

		let pending = model.pending.as_ref().map(|pending| pending.scene.id);
		let morph = model.morph().map(|(t, _, _, _)| t);
		draw.text(&format!(
			"scene {} unlocks [{}] pending {pending:?} morph {morph:?} ticks {:?}",
			model.scene.id,
			model.unlocks.names(),
			model.ticks
		))
		.x_y(-200.0, 140.0)
		.w(400.0);
	}

	draw.to_frame(app, &frame).unwrap();
}

// ---- the scene menu (M): the whole graph, click to jump; click an unlock to toggle it ----

const MENU_MARGIN: f32 = 20.0;
const MENU_ROW: f32 = 22.0;

/// Row height for the scene list: `MENU_ROW`, or less when the list wouldn't fit the canvas.
fn menu_row(window: Rect) -> f32 {
	let room = window.h() - 90.0 - MENU_MARGIN;
	MENU_ROW.min(room / scenes::all().len().max(1) as f32)
}
const MENU_UNLOCK_WIDTH: f32 = 63.0;

fn menu_unlocks_y(window: Rect) -> f32 {
	window.top() - 50.0
}

fn menu_scene_y(window: Rect, index: usize) -> f32 {
	window.top() - 90.0 - index as f32 * menu_row(window)
}

fn draw_menu(draw: &Draw, model: &Model, window: Rect) {
	draw.rect()
		.xy(window.xy())
		.wh(window.wh())
		.rgba(0.0, 0.0, 0.0, 0.85);

	let left = window.left() + MENU_MARGIN;
	let width = window.w() - 2.0 * MENU_MARGIN;
	let font = if menu_row(window) < MENU_ROW { 11 } else { 14 };
	let row = |text: &str, y: f32, gray: f32| {
		draw.text(text)
			.left_justify()
			.font_size(font)
			.x_y(left + width / 2.0, y)
			.w(width)
			.gray(gray);
	};

	row("M closes. Click a scene to jump there now; click an unlock to toggle it.", window.top() - 25.0, 0.5);

	let y = menu_unlocks_y(window);
	for (k, unlock) in Unlock::ALL.iter().enumerate() {
		let x = left + k as f32 * MENU_UNLOCK_WIDTH;
		let on = model.has(*unlock);
		draw.text(unlock.name())
			.left_justify()
			.font_size(13)
			.x_y(x + MENU_UNLOCK_WIDTH / 2.0, y)
			.w(MENU_UNLOCK_WIDTH)
			.gray(if on { 1.0 } else { 0.35 });
	}

	for (index, scene) in scenes::all().iter().enumerate() {
		let mut edges = vec![];
		for hotspot in scene.hotspots {
			let mut targets = vec![];
			scenes::targets(&hotspot.then, &mut targets);
			let mut grants = vec![];
			scenes::grants(&hotspot.then, &mut grants);
			let gesture = match hotspot.on {
				Gesture::Tap => "tap",
				Gesture::Drag { .. } => "drag",
				Gesture::Outline => "outline",
			};
			let mut edge = match &hotspot.then {
				Action::GoVia(_, bars) => format!("{gesture} via {bars:?}"),
				Action::Stage(stage) => format!("{gesture} -> stage {}", scene.stages.get(*stage).copied().unwrap_or("?")),
				_ => format!("{gesture}"),
			};
			if hotspot.needs != NONE {
				edge += &format!(" [{}]", hotspot.needs.names());
			}
			for unlock in grants {
				edge += &format!(" +{}", unlock.name());
			}
			for target in targets {
				edge += &format!(" -> {target}");
			}
			edges.push(edge);
		}
		let current = if std::ptr::eq(*scene, model.scene) { "> " } else { "  " };
		let text = format!(
			"{current}{}   ({} bars {:?}, loop from {})   {}",
			scene.id,
			scene.track.id,
			scene.bars,
			scene.loop_from,
			edges.join("  |  ")
		);
		row(&text, menu_scene_y(window, index), if current == "> " { 1.0 } else { 0.7 });
	}
}

/// A click while the menu is open. Returns whether it hit something.
fn menu_click(app: &App, model: &mut Model) {
	let window = app.window_rect();
	let (x, y) = (app.mouse.x, app.mouse.y);
	let left = window.left() + MENU_MARGIN;

	if (y - menu_unlocks_y(window)).abs() < MENU_ROW / 2.0 {
		let k = ((x - left) / MENU_UNLOCK_WIDTH).floor();
		if k >= 0.0 && (k as usize) < Unlock::ALL.len() {
			let unlock = Unlock::ALL[k as usize];
			let unlocks = if model.has(unlock) {
				model.unlocks.without(unlock)
			} else {
				model.unlocks.with(unlock)
			};
			model.set_unlocks(unlocks);
		}
		return;
	}

	let scenes = scenes::all();
	let row = menu_row(window);
	let k = ((menu_scene_y(window, 0) + row / 2.0 - y) / row).floor();
	if k >= 0.0 && (k as usize) < scenes.len() {
		model.menu = false;
		model.jump(scenes[k as usize]);
	}
}

// ---- input ----

pub fn key_pressed(_app: &App, model: &mut Model, key: Key) {
	match key {
		Key::M => model.menu = !model.menu,
		Key::Escape => model.menu = false,
		_ => {}
	}
}

pub fn mouse_pressed(app: &App, model: &mut Model, _button: MouseButton) {
	if model.menu {
		menu_click(app, model);
		return;
	}
	if !model.started {
		// The first click only starts the music.
		model.started = true;
		model.audio.resume();
		return;
	}
	if model.transitioning() {
		return;
	}
	let cell = cell_at(model, app.mouse.x, app.mouse.y);
	model.press = cell;
	model.trail_at = cell;
}

pub fn mouse_moved(app: &App, model: &mut Model, position: Point2) {
	if !app.mouse.buttons.left().is_down() || model.transitioning() || model.menu {
		return;
	}
	let Some(cell) = cell_at(model, position.x, position.y) else {
		return;
	};
	if let Some(last) = model.trail_at {
		if last != cell {
			model.draw_trail(last, cell, app.time);
		}
	}
	model.trail_at = Some(cell);
}

pub fn mouse_released(app: &App, model: &mut Model, _button: MouseButton) {
	model.trail_at = None;
	if model.trace.is_some() {
		// Let go before the outline was closed.
		model.abandon_trace(app.time);
	}
	let Some(press) = model.press.take() else {
		return;
	};
	let Some(release) = cell_at(model, app.mouse.x, app.mouse.y) else {
		return;
	};
	let hit = model.hotspots().find(|hotspot| {
		hotspot.covers(press.0, press.1)
			&& match hotspot.on {
				Gesture::Tap => hotspot.covers(release.0, release.1),
				Gesture::Drag { to } => to.contains(release.0, release.1),
				Gesture::Outline => false,
			}
	});
	if let Some(hotspot) = hit {
		model.act(&hotspot.then);
	}
}
