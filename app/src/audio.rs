//! Web Audio playback: each track is fetched and decoded once into an `AudioBuffer`; scenes
//! play as looped sections of it, and scene changes are scheduled on the audio clock so
//! they land exactly on a bar.
//!
//! Nothing is ever cut hard. A section is played one pass at a time, each pass its own
//! buffer source behind its own gain: it fades in over the last `FADE_IN` seconds before
//! its bar line (so the downbeat itself is at full level), holds, and fades out over
//! `LOOP_TAIL` after its last bar line, under the next pass. A scene change fades the new
//! section in the same way and fades whatever was playing out over `CUT_TAIL` from the
//! bar line. Passes are scheduled a little ahead of time (`LOOKAHEAD`) from `advance()`.

use {
	crate::scenes::{Scene, Track},
	js_sys::{Function, Reflect, Uint8Array},
	std::{
		cell::RefCell,
		collections::{HashMap, HashSet},
		ops::Range,
		rc::Rc,
	},
	wasm_bindgen::{JsCast, JsValue},
	wasm_bindgen_futures::{spawn_local, JsFuture},
	web_sys::{
		AnalyserNode, AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState,
		GainNode, Response,
	},
};

/// Seconds a pass takes to fade in, ending on its bar line.
const FADE_IN: f64 = 0.02;
/// Seconds a pass rings on past its last bar line, fading, under the next pass.
const LOOP_TAIL: f64 = 0.03;
/// Seconds the outgoing music takes to fade after the bar line a scene change lands on.
const CUT_TAIL: f64 = 0.08;
/// How far ahead of the clock passes are scheduled.
const LOOKAHEAD: f64 = 1.5;
/// Analyser window: `FFT_SIZE / 2` bins across 0..sample rate / 2. 2048 at 44.1 kHz is
/// 1024 bins of 21.5 Hz, fine enough for frequency bands the owner draws by hand.
const FFT_SIZE: usize = 2048;
/// The 16 coarse bins the old `modifiers` (bass/mid/treble) are made of.
pub const COARSE: usize = 16;

type Buffers = Rc<RefCell<HashMap<&'static str, AudioBuffer>>>;
/// Bytes received and bytes expected, per track being fetched.
type Progress = Rc<RefCell<HashMap<&'static str, (usize, usize)>>>;

/// One play-through of a section: a source behind a gain.
struct Pass {
	source: AudioBufferSourceNode,
	gain: GainNode,
	/// Context time of its bar line (where it reaches full level).
	start: f64,
	/// Seconds it plays (the first pass of a section may skip a lead).
	length: f64,
}

/// A section of a track, playing or scheduled: a scene's loop, or a transition passage.
struct Section {
	buffer: AudioBuffer,
	/// Seconds per bar of its track.
	bar: f64,
	/// Context time of its first bar line.
	start: f64,
	/// Buffer time of its first bar.
	offset: f64,
	/// Seconds per pass.
	length: f64,
	/// Seconds of the first pass skipped (the editor plays from the middle of a scene):
	/// a whole number of bars, so every pass still starts on a bar line.
	lead: f64,
	looping: bool,
	/// Context time it was cut at by a scene change, if it was.
	end: Option<f64>,
	passes: Vec<Pass>,
	/// Index of the next pass to schedule.
	next: usize,
}

impl Section {
	/// Context time pass `k` starts, and the buffer offset and seconds it plays.
	fn pass(&self, k: usize) -> (f64, f64, f64) {
		if k == 0 {
			(self.start, self.offset + self.lead, self.length - self.lead)
		} else {
			(
				self.start + (self.length - self.lead) + (k - 1) as f64 * self.length,
				self.offset,
				self.length,
			)
		}
	}

	fn done(&self, now: f64) -> bool {
		let last = self.passes.last().map_or(self.start, |pass| pass.start + pass.length);
		let more = self.looping && self.end.map_or(true, |end| last < end);
		!more && last + CUT_TAIL < now
	}
}

pub struct Audio {
	context: AudioContext,
	pub analyser: AnalyserNode,
	buffers: Buffers,
	loading: HashSet<&'static str>,
	progress: Progress,
	current: Option<Section>,
	/// In start order.
	queued: Vec<Section>,
	/// Cut, fading out, kept until they're over.
	fading: Vec<Section>,
}

impl Audio {
	pub fn new() -> Self {
		let context = AudioContext::new().unwrap();
		let analyser = AnalyserNode::new(&context).unwrap();
		analyser.set_fft_size(FFT_SIZE as u32);
		analyser.set_smoothing_time_constant(0.0);
		let _ = analyser.connect_with_audio_node(&context.destination());
		Audio {
			context,
			analyser,
			buffers: Default::default(),
			loading: Default::default(),
			progress: Default::default(),
			current: None,
			queued: vec![],
			fading: vec![],
		}
	}

	pub fn now(&self) -> f64 {
		self.context.current_time()
	}

	/// Browsers start the context suspended until a user gesture.
	pub fn resume(&self) {
		if self.context.state() != AudioContextState::Running {
			let _ = self.context.resume();
		}
	}

	/// Pause everything (the editor). `resume` continues.
	pub fn suspend(&self) {
		let _ = self.context.suspend();
	}

	pub fn is_suspended(&self) -> bool {
		self.context.state() != AudioContextState::Running
	}

	/// Hz per analyser bin.
	pub fn bin_hz(&self) -> f32 {
		self.context.sample_rate() / FFT_SIZE as f32
	}

	pub fn is_loaded(&self, track: &Track) -> bool {
		self.buffers.borrow().contains_key(track.id)
	}

	/// How much of a track has arrived, 0..1 — `None` until the fetch has started or the
	/// size is known. 1 means decoding (see `is_loaded`).
	pub fn progress(&self, track: &Track) -> Option<f32> {
		let progress = self.progress.borrow();
		let (got, total) = progress.get(track.id)?;
		(*total > 0).then(|| (*got as f32 / *total as f32).min(1.0))
	}

	/// Fetch and decode a track in the background, once.
	pub fn load(&mut self, track: &'static Track) {
		if self.is_loaded(track) || !self.loading.insert(track.id) {
			return;
		}
		let (context, buffers) = (self.context.clone(), self.buffers.clone());
		let progress = self.progress.clone();
		spawn_local(async move {
			match decode(&context, track, &progress).await {
				Ok(buffer) => {
					web_sys::console::log_1(
						&format!("loaded {}: {:.3} s at {} Hz", track.path, buffer.duration(), buffer.sample_rate()).into(),
					);
					buffers.borrow_mut().insert(track.id, buffer);
				}
				Err(error) => web_sys::console::error_2(
					&format!("failed to load {}", track.path).into(),
					&error,
				),
			}
		});
	}

	pub fn is_playing(&self) -> bool {
		self.current.is_some() || !self.queued.is_empty()
	}

	/// Every frame: schedule the passes due within `LOOKAHEAD`, promote whatever has
	/// begun, forget whatever is over.
	pub fn advance(&mut self) {
		let now = self.now();
		while self.queued.first().map_or(false, |queued| queued.start <= now) {
			if let Some(current) = self.current.take() {
				self.fading.push(current);
			}
			self.current = Some(self.queued.remove(0));
		}
		let context = self.context.clone();
		let analyser = self.analyser.clone();
		for section in self.current.iter_mut().chain(self.queued.iter_mut()) {
			loop {
				let k = section.next;
				if k > 0 && !section.looping {
					break;
				}
				let (start, offset, length) = section.pass(k);
				if start > now + LOOKAHEAD || section.end.map_or(false, |end| start >= end) {
					break;
				}
				match schedule(&context, &analyser, &section.buffer, start, offset, length) {
					Ok(pass) => section.passes.push(pass),
					Err(error) => web_sys::console::error_2(
						&format!("couldn't schedule a pass at {start:.3}").into(),
						&error,
					),
				}
				section.next += 1;
			}
			section
				.passes
				.retain(|pass| pass.start + pass.length + CUT_TAIL >= now);
		}
		self.fading.retain(|section| !section.done(now));
	}

	/// Context time of the next bar boundary (now, if nothing is playing).
	pub fn next_bar(&self) -> f64 {
		let now = self.now();
		match &self.current {
			Some(section) => {
				let bars = ((now - section.start) / section.bar).floor() + 1.0;
				section.start + bars * section.bar
			}
			None => now,
		}
	}

	/// Bar phase (0..1) of what's playing.
	pub fn phase(&self) -> Option<f64> {
		let section = self.current.as_ref()?;
		Some(((self.now() - section.start) / section.bar).rem_euclid(1.0))
	}

	/// Where on its track what's playing is, in bars (fractional): the section's first bar
	/// plus how far into it, wrapped if it loops.
	pub fn bar(&self) -> Option<f64> {
		let section = self.current.as_ref()?;
		let first = section.offset / section.bar;
		let into = (section.lead + (self.now() - section.start).max(0.0)) / section.bar;
		let bars = section.length / section.bar;
		Some(first + if section.looping && bars > 0.0 { into.rem_euclid(bars) } else { into })
	}

	/// The editor: play `scene`'s stretch from `bar` (one of its bars) at context time `at`.
	/// Fails if the track isn't decoded.
	pub fn play_from(&mut self, scene: &Scene, bar: usize, at: f64) -> Result<f64, ()> {
		let lead = bar.saturating_sub(scene.bars.start).min(scene.bars.len().saturating_sub(1));
		self.start(scene, None, at, lead)
	}

	/// Schedule `scene`'s stretch to start at context time `at`, or, with `via`, after those
	/// bars of that track have played once from `at`. Whatever else is playing fades out
	/// from `at`. Returns the context time the stretch itself starts. Fails if a track
	/// isn't decoded yet.
	pub fn play(
		&mut self,
		scene: &Scene,
		via: Option<(&Track, Range<usize>)>,
		at: f64,
	) -> Result<f64, ()> {
		self.start(scene, via, at, 0)
	}

	fn start(
		&mut self,
		scene: &Scene,
		via: Option<(&Track, Range<usize>)>,
		at: f64,
		lead: usize,
	) -> Result<f64, ()> {
		let get = |track: &Track| self.buffers.borrow().get(track.id).cloned().ok_or(());
		let buffer = get(scene.track)?;

		let mut sections = vec![];
		let mut start = at;
		if let Some((track, bars)) = via {
			let bar = track.bar();
			let length = bars.len() as f64 * bar;
			sections.push(Section {
				buffer: get(track)?,
				bar,
				start,
				offset: bars.start as f64 * bar,
				length,
				lead: 0.0,
				looping: false,
				end: None,
				passes: vec![],
				next: 0,
			});
			start += length;
		}
		// The stretch: its first bars once (skipping `lead` of them), then the tail looping.
		let bar = scene.track.bar();
		let landing = start;
		let loop_from = scene.loop_from.clamp(scene.bars.start, scene.bars.end.saturating_sub(1));
		let intro = loop_from - scene.bars.start;
		let lead = lead.min(scene.bars.len().saturating_sub(1));
		if lead < intro {
			let length = (intro - lead) as f64 * bar;
			sections.push(Section {
				buffer: buffer.clone(),
				bar,
				start,
				offset: (scene.bars.start + lead) as f64 * bar,
				length,
				lead: 0.0,
				looping: false,
				end: None,
				passes: vec![],
				next: 0,
			});
			start += length;
		}
		sections.push(Section {
			buffer,
			bar,
			start,
			offset: loop_from as f64 * bar,
			length: (scene.bars.end - loop_from) as f64 * bar,
			lead: lead.saturating_sub(intro) as f64 * bar,
			looping: true,
			end: None,
			passes: vec![],
			next: 0,
		});
		let start = landing;

		for section in self.current.iter_mut().chain(self.queued.iter_mut()) {
			cut(section, at);
		}
		self.fading.extend(self.current.take());
		self.fading.append(&mut self.queued);
		self.queued = sections;
		self.advance();
		Ok(start)
	}

	/// The 16 frequency bins of what's playing right now, 0–255.
	pub fn fft(&self) -> [u8; COARSE] {
		coarse(&self.spectrum())
	}

	/// The whole spectrum, `FFT_SIZE / 2` bins of `bin_hz()` each, 0-255.
	pub fn spectrum(&self) -> Vec<u8> {
		let mut bins = vec![0; FFT_SIZE / 2];
		self.analyser.get_byte_frequency_data(&mut bins);
		bins
	}
}

/// One pass of `section` with its bar line at `start`: fades in over `FADE_IN` before it,
/// holds for the section's length, rings out over `LOOP_TAIL`. (Nothing may be scheduled
/// before context time 0 — Web Audio throws — so the very first pass gets no lead-in.)
fn schedule(
	context: &AudioContext,
	analyser: &AnalyserNode,
	buffer: &AudioBuffer,
	start: f64,
	offset: f64,
	length: f64,
) -> Result<Pass, JsValue> {
	let pre = FADE_IN.min(offset).min(start).max(0.0);
	let source = AudioBufferSourceNode::new(context)?;
	source.set_buffer(Some(buffer));
	let gain = GainNode::new(context)?;
	source.connect_with_audio_node(&gain)?;
	gain.connect_with_audio_node(analyser)?;
	let param = gain.gain();
	param.set_value_at_time(0.0, start - pre)?;
	param.linear_ramp_to_value_at_time(1.0, start)?;
	param.set_value_at_time(1.0, start + length)?;
	param.linear_ramp_to_value_at_time(0.0, start + length + LOOP_TAIL)?;
	source.start_with_when_and_grain_offset_and_grain_duration(
		start - pre,
		offset - pre,
		pre + length + LOOP_TAIL,
	)?;
	Ok(Pass {
		source,
		gain,
		start,
		length,
	})
}

/// Fade `section` out from `at`, a bar line: whatever pass is sounding there is at full
/// level, so hold 1 at `at` and ramp down over `CUT_TAIL`; passes due after `at` never
/// start.
fn cut(section: &mut Section, at: f64) {
	section.end = Some(at);
	for pass in &section.passes {
		if pass.start >= at {
			let _ = pass.source.stop_with_when(at.max(0.0));
			continue;
		}
		let param = pass.gain.gain();
		let _ = param.cancel_scheduled_values(at);
		let _ = param.set_value_at_time(1.0, at);
		let _ = param.linear_ramp_to_value_at_time(0.0, at + CUT_TAIL);
		let _ = pass.source.stop_with_when(at + CUT_TAIL);
	}
}

/// Fetch a track, counting the bytes as they come (for the loading bar), then decode it.
async fn decode(
	context: &AudioContext,
	track: &'static Track,
	progress: &Progress,
) -> Result<AudioBuffer, JsValue> {
	let window = web_sys::window().ok_or("no window")?;
	let response: Response = JsFuture::from(window.fetch_with_str(track.path))
		.await?
		.dyn_into()?;
	if !response.ok() {
		return Err(format!("{} {}", response.status(), response.status_text()).into());
	}
	let total = response
		.headers()
		.get("content-length")?
		.and_then(|length| length.parse::<usize>().ok())
		.unwrap_or(0);
	progress.borrow_mut().insert(track.id, (0, total));
	// The stream reader, untyped: its `read()` resolves to `{ done, value }`.
	let reader = response.body().ok_or("no body")?.get_reader();
	let read: Function = Reflect::get(&reader, &"read".into())?.dyn_into()?;
	let mut bytes: Vec<u8> = Vec::with_capacity(total);
	loop {
		let chunk = JsFuture::from(read.call0(&reader)?.dyn_into::<js_sys::Promise>()?).await?;
		if Reflect::get(&chunk, &"done".into())?.is_truthy() {
			break;
		}
		let value: Uint8Array = Reflect::get(&chunk, &"value".into())?.dyn_into()?;
		bytes.extend(value.to_vec());
		progress
			.borrow_mut()
			.insert(track.id, (bytes.len(), total.max(bytes.len())));
	}
	progress
		.borrow_mut()
		.insert(track.id, (bytes.len(), bytes.len()));
	let array = Uint8Array::from(bytes.as_slice());
	let decoded = JsFuture::from(context.decode_audio_data(&array.buffer())?).await?;
	Ok(decoded.dyn_into()?)
}

/// The spectrum folded into `COARSE` equal bins (what a 32-point FFT would have given).
pub fn coarse(bins: &[u8]) -> [u8; COARSE] {
	let mut out = [0; COARSE];
	let per = (bins.len() / COARSE).max(1);
	for (k, slot) in out.iter_mut().enumerate() {
		let chunk = &bins[k * per..((k + 1) * per).min(bins.len())];
		if !chunk.is_empty() {
			*slot = (chunk.iter().map(|&b| b as u32).sum::<u32>() / chunk.len() as u32) as u8;
		}
	}
	out
}
