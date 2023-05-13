use {
	crate::scenes::*,
	nannou::{
		noise::{
			utils::{NoiseMap, NoiseMapBuilder, PlaneMapBuilder},
			OpenSimplex, RidgedMulti, Worley,
		},
		prelude::*,
	},
	web_sys::{AnalyserNode, AudioContext, HtmlAudioElement},
};

pub const CANVAS_WIDTH: usize = 20;
pub const CANVAS_HEIGHT: usize = 20;
const SCALING: usize = 40;

pub struct Audio {
	pub analyser: AnalyserNode,
	context: AudioContext,
	elements: [HtmlAudioElement; 2],
	current_element: usize,
}

pub struct Model {
	pub noise_matrix: RidgedMulti,
	pub noisemap: NoiseMap,
	pub scaling: usize,
	pub audio: Audio,

	scenes: Vec<&'static Scene>,
	queue_next: bool,
	current_scene: usize,
	current_measure: usize,
	pub last_whole: f64,
}

impl Model {
	pub fn new() -> Self {
		let mut scenes = vec![];
		scenes.extend_from_slice(for_against::SCENES);
		scenes.extend_from_slice(terra_firmament::SCENES);
		let context = AudioContext::new().unwrap();
		let analyser = AnalyserNode::new(&context).unwrap();
		analyser.set_fft_size(32);
		let element1 = HtmlAudioElement::new_with_src(scenes[0].path).unwrap();
		let element2 = HtmlAudioElement::new_with_src(scenes[1].path).unwrap();

		let _ = context
			.create_media_element_source(&element1)
			.unwrap()
			.connect_with_audio_node(&analyser);
		let _ = context
			.create_media_element_source(&element2)
			.unwrap()
			.connect_with_audio_node(&analyser);
		let _ = analyser.connect_with_audio_node(&context.destination());

		let audio = Audio {
			analyser,
			context,
			elements: [element1, element2],
			current_element: 0,
		};
		Model {
			scaling: SCALING,
			noise_matrix: RidgedMulti::new(),
			noisemap: PlaneMapBuilder::new(&RidgedMulti::new())
				.set_size(CANVAS_WIDTH, CANVAS_HEIGHT)
				.set_x_bounds(-10.0, 10.0)
				.set_y_bounds(-10.0, 10.0)
				.build(),
			scenes,
			queue_next: false,
			current_scene: 0,
			current_measure: 0,
			audio,
			last_whole: 0.0,
		}
	}

	pub fn current_scene(&self) -> &Scene {
		self.scenes[self.current_scene]
	}

	fn play(&self) {
		let _ = self.audio.context.resume();
		let element = &self.audio.elements[self.audio.current_element];
		if element.ready_state() != 4 {
			return;
		}
		if element.paused() {
			element.set_loop(self.scenes[self.current_scene].loop_);
			let _ = element.play();
		}
	}

	fn stop(&self) {
		let _ = self.audio.elements[self.audio.current_element].pause();
	}
}

pub fn update(_app: &App, model: &mut Model, _update: Update) {
	let scene = &model.scenes[model.current_scene];
	let element = &model.audio.elements[model.audio.current_element];
	let measure_duration = element.duration() / scene.measures as f64;
	let mut last_whole = (element.current_time() % measure_duration) / measure_duration;
	if last_whole < model.last_whole {
		model.current_measure += 1;

		if model.current_measure == scene.measures {
			model.current_measure = 0;
			if !scene.loop_ {
				model.queue_next = true;
			}
		}

		if model.queue_next {
			model.queue_next = false;

			if model.current_scene < model.scenes.len() - 1 {
				model.stop();
				model.current_scene += 1;
				model.current_measure = 0;

				let element =
					HtmlAudioElement::new_with_src(model.scenes[model.current_scene + 1].path)
						.unwrap();
				let _ = (model.audio.context)
					.create_media_element_source(&element)
					.unwrap()
					.connect_with_audio_node(&model.audio.context.destination());

				model.audio.elements[model.audio.current_element] = element;
				model.audio.current_element = model.audio.current_element ^ 1;
				model.play();
				last_whole = 0.0;
			}
		}
	}

	model.last_whole = last_whole;
}

pub fn view(app: &App, model: &Model, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let modifiers = get_modifiers(model);

	// debug info
	{
		draw.text(&format!("Fps: {:?}", app.fps().round()))
			.x_y(-200.0, 220.0);

		let [whole, half, quarter, eighth, sixteenth, bass, mid, treble] = modifiers;

		draw.text(&format!(
			"1({:.2}) 2({half:.2}) 4({quarter:.2}) 8({eighth:.2}) 16({sixteenth:.2})",
			model.last_whole
		))
		.x_y(-200.0, 180.0);

		draw.text(&format!(
			"bass({bass:.2}) mid({mid:.2}) treble({treble:.2})"
		))
		.x_y(-200.0, 160.0);

		let mut fft = [0; 16];
		model.audio.analyser.get_byte_frequency_data(&mut fft);
		let fft = fft
			.iter()
			.map(|&value| format!("{:.2}|", value as f32 / 255.0))
			.collect::<String>();
		draw.text(&format!("FFT: {fft}")).x_y(200.0, 160.0);
	}

	if model.audio.elements[model.audio.current_element].ready_state() != 4 {
		return;
	}

	for i in 0..CANVAS_WIDTH {
		for j in 0..CANVAS_HEIGHT {
			(model.scenes[model.current_scene].render)(
				&draw,
				model,
				i,
				j,
				app.elapsed_frames() as f32 / 100.0,
				modifiers,
			);
		}
	}

	draw.to_frame(app, &frame).unwrap();
}

pub fn mouse_pressed(app: &App, model: &mut Model, _button: MouseButton) {
	model.play();

	if model.scenes[model.current_scene].boxes[0].inside(app.mouse.x, app.mouse.y) {
		model.queue_next = true;
	}
}
