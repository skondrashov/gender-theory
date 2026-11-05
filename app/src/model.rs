use {
	crate::{
		audio::Audio,
		progress,
		scenes::{self, *},
	},
	nannou::{
		noise::{
			utils::{NoiseMap, NoiseMapBuilder, PlaneMapBuilder},
			RidgedMulti,
		},
		prelude::*,
	},
	std::{
		cell::{Cell, RefCell},
		collections::HashSet,
		ops::Range,
	},
};

pub const CANVAS_WIDTH: usize = 20;
pub const CANVAS_HEIGHT: usize = 20;
const SCALING: usize = 40;
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
	pub scaling: usize,
	pub audio: Audio,
	pub debug: bool,

	scene: &'static Scene,
	unlocks: Unlocks,
	/// When each unlock was granted (app time), for easing it in. `-inf` = always had it.
	revealed: [f32; 5],
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
}

impl Model {
	/// `start` is a scene id to begin on (`?scene=`), overriding saved progress; `unlock` is
	/// unlock names joined by `+` (`?unlock=lines+rotation`) to grant on top of it.
	pub fn new(debug: bool, start: Option<String>, reset: bool, unlock: Option<String>) -> Self {
		if reset {
			progress::clear();
		}
		let saved = progress::load();
		let mut unlocks = saved.as_ref().map_or(NONE, |(_, unlocks)| *unlocks);
		for name in unlock.as_deref().unwrap_or_default().split('+') {
			if let Some(unlock) = Unlock::ALL.iter().find(|unlock| unlock.name() == name) {
				unlocks = unlocks.with(*unlock);
			}
		}
		let first = *scenes::all().first().expect("no scenes");
		let scene = start
			.as_deref()
			.or(saved.as_ref().map(|(scene, _)| scene.as_str()))
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
			scene,
			unlocks,
			revealed: [f32::NEG_INFINITY; 5],
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
		};
		model.preload();
		model
	}

	pub fn current_scene(&self) -> &'static Scene {
		self.scene
	}

	/// The scene being rendered: the one we're on, or the one being morphed into.
	fn viewed(&self) -> &'static Scene {
		self.preview.get().unwrap_or(self.scene)
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
		t * t * (3.0 - 2.0 * t)
	}

	/// The `tag` of the unlocked hotspot covering cell (i, j), if any. Scenes draw hints with this.
	pub fn hint_at(&self, i: usize, j: usize) -> Option<u8> {
		self.hotspots()
			.find(|hotspot| hotspot.cells.contains(i, j))
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
	pub fn morph(&self) -> Option<(f32, &'static Scene, f32)> {
		let pending = self.pending.as_ref()?;
		let lands = pending.lands?;
		let now = self.audio.now();
		let start = pending.asked.max(lands - MORPH);
		if now < start || lands <= start {
			return None;
		}
		let t = ((now - start) / (lands - start)).clamp(0.0, 1.0) as f32;
		Some((t * t * (3.0 - 2.0 * t), pending.scene, (lands - now).max(0.0) as f32))
	}

	/// Unlocked, and still has something to do.
	fn visible(&self, hotspot: &Hotspot) -> bool {
		self.unlocks.has_all(hotspot.needs) && !hotspot.then.spent(self.unlocks)
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
		progress::save(self.scene.id, self.unlocks);
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

	/// Go somewhere right now, bar or no bar (the scene menu).
	fn jump(&mut self, scene: &'static Scene) {
		self.started = true;
		self.audio.resume();
		self.audio.load(scene.track);
		if self.audio.is_playing() {
			self.pending = Some(Pending {
				scene,
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
		self.ticks = [0; 5];
		self.phases = [0.0; 5];
		self.trail.clear();
		self.trace = None;
		progress::save(scene.id, self.unlocks);
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

pub fn update(app: &App, model: &mut Model, _update: Update) {
	let now = model.audio.now();
	model.time = app.time;

	model.audio.advance();
	if let Some(pending) = &model.pending {
		if pending.lands.map_or(false, |lands| now >= lands) {
			let scene = pending.scene;
			model.pending = None;
			model.enter(scene);
		}
	}

	if model.started {
		model.audio.resume();
		if !model.audio.is_playing() {
			// Retried every frame until the track is decoded.
			let _ = model.audio.play(model.scene, None, now);
		}
	}

	// Schedule a pending change once its track is decoded: at the next bar, or a morph
	// away for the menu. A via passage is bars of the track we're on.
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
			let via = pending.via.clone().map(|bars| (model.scene.track, bars));
			if let Ok(lands) = model.audio.play(pending.scene, via, at) {
				pending.at = Some(at);
				pending.lands = Some(lands);
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
}

pub fn view(app: &App, model: &Model, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let modifiers = get_modifiers(model);

	if model.audio.is_loaded(model.scene.track) {
		let time = app.time * TIME_RATE;
		let morph = model.morph();
		for i in 0..CANVAS_WIDTH {
			for j in 0..CANVAS_HEIGHT {
				(model.scene.render)(&draw, model, i, j, time, modifiers);
				let marks = model.take_marks();
				match morph {
					None => marks.iter().for_each(|mark| mark.draw(&draw)),
					Some((t, next, left)) => {
						// Where the next scene will be as it lands: the top of its bar,
						// `left` seconds from now, the music as it is.
						let mut landing = [0.0; 8];
						landing[5..].copy_from_slice(&modifiers[5..]);
						model.preview.set(Some(next));
						(next.render)(&draw, model, i, j, time + left * TIME_RATE, landing);
						model.preview.set(None);
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

	// Something has been set in motion: a white border until it lands.
	if model.transitioning() {
		draw.rect()
			.no_fill()
			.stroke_color(WHITE)
			.stroke_weight(8.0)
			.xy(window.xy())
			.wh(window.wh());
	}

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
		let morph = model.morph().map(|(t, _, _)| t);
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
const MENU_UNLOCK_WIDTH: f32 = 90.0;

fn menu_unlocks_y(window: Rect) -> f32 {
	window.top() - 50.0
}

fn menu_scene_y(window: Rect, index: usize) -> f32 {
	window.top() - 90.0 - index as f32 * MENU_ROW
}

fn draw_menu(draw: &Draw, model: &Model, window: Rect) {
	draw.rect()
		.xy(window.xy())
		.wh(window.wh())
		.rgba(0.0, 0.0, 0.0, 0.85);

	let left = window.left() + MENU_MARGIN;
	let width = window.w() - 2.0 * MENU_MARGIN;
	let row = |text: &str, y: f32, gray: f32| {
		draw.text(text)
			.left_justify()
			.font_size(14)
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
			.font_size(14)
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
			"{current}{}   ({} bars {:?})   {}",
			scene.id,
			scene.track.id,
			scene.bars,
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
	let k = ((menu_scene_y(window, 0) + MENU_ROW / 2.0 - y) / MENU_ROW).floor();
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
		hotspot.cells.contains(press.0, press.1)
			&& match hotspot.on {
				Gesture::Tap => hotspot.cells.contains(release.0, release.1),
				Gesture::Drag { to } => to.contains(release.0, release.1),
				Gesture::Outline => false,
			}
	});
	if let Some(hotspot) = hit {
		model.act(&hotspot.then);
	}
}
