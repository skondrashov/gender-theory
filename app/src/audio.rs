//! Web Audio playback: each track is fetched and decoded once into an `AudioBuffer`; scenes
//! play as buffer sources with sample-accurate loop points, and scene changes are scheduled
//! on the audio clock so they land exactly on a bar.

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
		Response,
	},
};

type Buffers = Rc<RefCell<HashMap<&'static str, AudioBuffer>>>;

/// Something playing or scheduled: a scene's loop, or a transition passage before one.
struct Playing {
	sources: Vec<AudioBufferSourceNode>,
	/// Seconds per bar of its track.
	bar: f64,
	/// Context time it starts.
	start: f64,
}

pub struct Audio {
	context: AudioContext,
	pub analyser: AnalyserNode,
	buffers: Buffers,
	loading: HashSet<&'static str>,
	current: Option<Playing>,
	/// In start order.
	queued: Vec<Playing>,
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

	/// Promote whatever scheduled audio has begun.
	pub fn advance(&mut self) {
		let now = self.now();
		while self.queued.first().map_or(false, |queued| queued.start <= now) {
			self.current = Some(self.queued.remove(0));
		}
	}

	/// Context time of the next bar boundary (now, if nothing is playing).
	pub fn next_bar(&self) -> f64 {
		let now = self.now();
		match &self.current {
			Some(playing) => {
				let bars = ((now - playing.start) / playing.bar).floor() + 1.0;
				playing.start + bars * playing.bar
			}
			None => now,
		}
	}

	/// Bar phase (0..1) of what's playing.
	pub fn phase(&self) -> Option<f64> {
		let playing = self.current.as_ref()?;
		Some(((self.now() - playing.start) / playing.bar).rem_euclid(1.0))
	}

	/// Schedule `scene`'s loop to start at context time `at`, or, with `via`, after those
	/// bars of that track have played once from `at`. Whatever else is playing stops at
	/// `at`. Returns the context time the loop itself starts. Fails if a track isn't
	/// decoded yet.
	pub fn play(
		&mut self,
		scene: &Scene,
		via: Option<(&Track, Range<usize>)>,
		at: f64,
	) -> Result<f64, ()> {
		let get = |track: &Track| self.buffers.borrow().get(track.id).cloned().ok_or(());
		let buffer = get(scene.track)?;

		let mut playings = vec![];
		let mut section = at;
		if let Some((track, bars)) = via {
			let via_buffer = get(track)?;
			let bar = via_buffer.duration() / track.measures as f64;
			let length = bars.len() as f64 * bar;
			let source = self.source(&via_buffer, bars.start as f64 * bar, Some(length), None, at)?;
			playings.push(Playing {
				sources: vec![source],
				bar,
				start: at,
			});
			section += length;
		}

		let bar = buffer.duration() / scene.track.measures as f64;
		let (offset, length) = (scene.bars.start as f64 * bar, scene.bars.len() as f64 * bar);
		let source = self.source(&buffer, offset, None, Some((offset, offset + length)), section)?;
		playings.push(Playing {
			sources: vec![source],
			bar,
			start: section,
		});

		for playing in self.current.iter().chain(self.queued.iter()) {
			for source in &playing.sources {
				let _ = source.stop_with_when(at);
			}
		}
		self.queued = playings;
		Ok(section)
	}

	fn source(
		&self,
		buffer: &AudioBuffer,
		offset: f64,
		duration: Option<f64>,
		loop_: Option<(f64, f64)>,
		at: f64,
	) -> Result<AudioBufferSourceNode, ()> {
		let source = AudioBufferSourceNode::new(&self.context).map_err(|_| ())?;
		source.set_buffer(Some(buffer));
		if let Some((start, end)) = loop_ {
			source.set_loop(true);
			source.set_loop_start(start);
			source.set_loop_end(end);
		}
		source
			.connect_with_audio_node(&self.analyser)
			.map_err(|_| ())?;
		match duration {
			Some(duration) => {
				source.start_with_when_and_grain_offset_and_grain_duration(at, offset, duration)
			}
			None => source.start_with_when_and_grain_offset(at, offset),
		}
		.map_err(|_| ())?;
		Ok(source)
	}

	/// The 16 frequency bins of what's playing right now, 0–255.
	pub fn fft(&self) -> [u8; 16] {
		let mut fft = [0; 16];
		self.analyser.get_byte_frequency_data(&mut fft);
		fft
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
