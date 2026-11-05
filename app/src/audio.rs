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
	js_sys::ArrayBuffer,
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

type Buffers = Rc<RefCell<HashMap<&'static str, AudioBuffer>>>;

/// One play-through of a section: a source behind a gain.
struct Pass {
	source: AudioBufferSourceNode,
	gain: GainNode,
	/// Context time of its bar line (where it reaches full level).
	start: f64,
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
	looping: bool,
	/// Context time it was cut at by a scene change, if it was.
	end: Option<f64>,
	passes: Vec<Pass>,
	/// Index of the next pass to schedule.
	next: usize,
}

impl Section {
	fn done(&self, now: f64) -> bool {
		let last = self.passes.last().map_or(self.start, |pass| pass.start + self.length);
		let more = self.looping && self.end.map_or(true, |end| last < end);
		!more && last + CUT_TAIL < now
	}
}

pub struct Audio {
	context: AudioContext,
	pub analyser: AnalyserNode,
	buffers: Buffers,
	loading: HashSet<&'static str>,
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
		analyser.set_fft_size(32);
		let _ = analyser.connect_with_audio_node(&context.destination());
		Audio {
			context,
			analyser,
			buffers: Default::default(),
			loading: Default::default(),
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

	pub fn is_loaded(&self, track: &Track) -> bool {
		self.buffers.borrow().contains_key(track.id)
	}

	/// Fetch and decode a track in the background, once.
	pub fn load(&mut self, track: &'static Track) {
		if self.is_loaded(track) || !self.loading.insert(track.id) {
			return;
		}
		let (context, buffers) = (self.context.clone(), self.buffers.clone());
		spawn_local(async move {
			match decode(&context, track.path).await {
				Ok(buffer) => {
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
				let start = section.start + k as f64 * section.length;
				if start > now + LOOKAHEAD || section.end.map_or(false, |end| start >= end) {
					break;
				}
				if let Ok(pass) = schedule(&context, &analyser, section, start) {
					section.passes.push(pass);
				}
				section.next += 1;
			}
			section
				.passes
				.retain(|pass| pass.start + section.length + CUT_TAIL >= now);
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

	/// Schedule `scene`'s loop to start at context time `at`, or, with `via`, after those
	/// bars of that track have played once from `at`. Whatever else is playing fades out
	/// from `at`. Returns the context time the loop itself starts. Fails if a track isn't
	/// decoded yet.
	pub fn play(
		&mut self,
		scene: &Scene,
		via: Option<(&Track, Range<usize>)>,
		at: f64,
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
				looping: false,
				end: None,
				passes: vec![],
				next: 0,
			});
			start += length;
		}
		let bar = scene.track.bar();
		sections.push(Section {
			buffer,
			bar,
			start,
			offset: scene.bars.start as f64 * bar,
			length: scene.bars.len() as f64 * bar,
			looping: true,
			end: None,
			passes: vec![],
			next: 0,
		});

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
	pub fn fft(&self) -> [u8; 16] {
		let mut fft = [0; 16];
		self.analyser.get_byte_frequency_data(&mut fft);
		fft
	}
}

/// One pass of `section` with its bar line at `start`: fades in over `FADE_IN` before it,
/// holds for the section's length, rings out over `LOOP_TAIL`.
fn schedule(
	context: &AudioContext,
	analyser: &AnalyserNode,
	section: &Section,
	start: f64,
) -> Result<Pass, JsValue> {
	let pre = FADE_IN.min(section.offset);
	let source = AudioBufferSourceNode::new(context)?;
	source.set_buffer(Some(&section.buffer));
	let gain = GainNode::new(context)?;
	source.connect_with_audio_node(&gain)?;
	gain.connect_with_audio_node(analyser)?;
	let param = gain.gain();
	param.set_value_at_time(0.0, start - pre)?;
	param.linear_ramp_to_value_at_time(1.0, start)?;
	param.set_value_at_time(1.0, start + section.length)?;
	param.linear_ramp_to_value_at_time(0.0, start + section.length + LOOP_TAIL)?;
	source.start_with_when_and_grain_offset_and_grain_duration(
		start - pre,
		section.offset - pre,
		pre + section.length + LOOP_TAIL,
	)?;
	Ok(Pass {
		source,
		gain,
		start,
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

async fn decode(context: &AudioContext, path: &str) -> Result<AudioBuffer, JsValue> {
	let window = web_sys::window().ok_or("no window")?;
	let response: Response = JsFuture::from(window.fetch_with_str(path))
		.await?
		.dyn_into()?;
	let bytes: ArrayBuffer = JsFuture::from(response.array_buffer()?)
		.await?
		.dyn_into()?;
	let decoded = JsFuture::from(context.decode_audio_data(&bytes)?).await?;
	Ok(decoded.dyn_into()?)
}
