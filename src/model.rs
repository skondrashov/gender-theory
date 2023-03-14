use {
	crate::notes::NOTES,
	nannou::{
		noise::{
			utils::{NoiseMap, NoiseMapBuilder, PlaneMapBuilder},
			NoiseFn, OpenSimplex, Worley,
		},
		prelude::*,
		wgpu::{Backends, DeviceDescriptor, Limits},
	},
	std::{cell::RefCell, time::Duration},
	web_sys::{console, HtmlAudioElement},
};

const WEB_TARGET: bool = true;

pub const NOTES_PER_MEASURE: usize = 16;
pub const EFFECTS: usize = 4;

const CANVAS_WIDTH: usize = 20;
const CANVAS_HEIGHT: usize = 10;
const SCALING: usize = 40;
// const FRAME_RATE: u64 = 24;
// const NOISE_LENGTH: f64 = 3.0;

const FPS: f64 = 2.0;

struct NativeAudio {}
struct HtmlAudio {
	elements: [HtmlAudioElement; 2],
	current_element: usize,
}
enum Audio {
	Html(HtmlAudio),
	_Native(NativeAudio),
	None,
}

impl Model {
	fn play(&self) {
		if let Audio::Html(HtmlAudio {
			elements,
			current_element,
		}) = &self.audio
		{
			let element = &elements[*current_element];
			if element.ready_state() != 4 {
				return;
			}
			if element.paused() {
				element.set_loop(self.scenes[self.current_scene].loop_);
				let _ = element.play().unwrap();
			}
		} else {
			()
		}
	}
	fn stop(&self) {
		if let Audio::Html(HtmlAudio {
			elements,
			current_element,
		}) = &self.audio
		{
			elements[*current_element].pause().unwrap();
		} else {
			()
		}
	}
	fn next(&mut self) {
		if let Audio::Html(audio) = &mut self.audio {
			audio.current_element = !audio.current_element;
		}
		self.stop();
		self.play();
	}
}

struct Area {
	x0: f32,
	x1: f32,
	y0: f32,
	y1: f32,
}

impl Area {
	fn inside(&self, x: f32, y: f32) -> bool {
		self.x0 < x && x < self.x1 && self.y0 < y && y < self.y1
	}
}

struct Scene {
	path: &'static str,
	loop_: bool,
	measures: usize,
	notes: [[bool; EFFECTS]; NOTES_PER_MEASURE],
	boxes: [Area; 2],
}

pub struct Model {
	previous_render_time: Duration,

	modifiers: [(f32, usize); EFFECTS],
	values: [f32; EFFECTS],

	noise_matrix: OpenSimplex, // [[[f32; CANVAS_HEIGHT]; CANVAS_WIDTH]; KEYFRAMES],
	noisemap: NoiseMap,

	scaling: usize,
	audio: Audio,
	scenes: Vec<Scene>,
	current_scene: usize,
	current_measure: usize,
	current_note: usize,
	advance_scene: bool,
}

impl Model {
	pub fn new() -> Self {
		let scenes = vec![
			Scene {
				path: "/fa/1.mp3",
				loop_: true,
				measures: 14,
				notes: NOTES[0],
				boxes: [
					Area {
						x0: -300.0,
						x1: -300.0,
						y0: 300.0,
						y1: 300.0,
					},
					Area {
						x0: 0.0,
						x1: 0.0,
						y0: 0.0,
						y1: 0.0,
					},
				],
			},
			Scene {
				path: "/fa/2.mp3",
				loop_: true,
				measures: 14,
				notes: NOTES[1],
				boxes: [
					Area {
						x0: 0.0,
						x1: 0.0,
						y0: 0.0,
						y1: 0.0,
					},
					Area {
						x0: 0.0,
						x1: 0.0,
						y0: 0.0,
						y1: 0.0,
					},
				],
			},
		];
		let audio = if WEB_TARGET {
			Audio::Html(HtmlAudio {
				elements: [
					HtmlAudioElement::new_with_src(scenes[0].path).unwrap(),
					HtmlAudioElement::new_with_src(scenes[1].path).unwrap(),
				],
				current_element: 0,
			})
		} else {
			Audio::None
		};
		Model {
			previous_render_time: Duration::new(0, 0),

			scaling: SCALING,
			values: [0.0, 0.0, 0.0, 0.0],
			modifiers: [(5.5, 0), (5.2, 1), (5.1, 2), (5.0, 3)],
			noise_matrix: OpenSimplex::new(),
			noisemap: PlaneMapBuilder::new(&Worley::new())
				.set_size(CANVAS_WIDTH, CANVAS_HEIGHT)
				.set_x_bounds(-10.0, 10.0)
				.set_y_bounds(-10.0, 10.0)
				.build(),
			scenes,
			current_scene: 0,
			current_measure: 0,
			current_note: 0,
			audio,
			advance_scene: false,
		}
	}
}

fn update(app: &App, model: &mut Model, _update: Update) {
	// if previous_note_time <= update.since_start - Duration::from_secs_f64(1.0 / FRAME_RATE as f64) {
	// }
	// if app.duration.since_start % 60 != 0 {
	//     return;
	// }

	let scene = &model.scenes[model.current_scene];
	if let Audio::Html(HtmlAudio {
		elements,
		current_element,
	}) = &mut model.audio
	{
		let element = &elements[*current_element];
		if element.paused() {
			return;
		}
		let new_current_note = (0.5
			+ (NOTES_PER_MEASURE * scene.measures) as f64
				* (element.current_time() / element.duration())) as usize;

		if model.current_note < new_current_note {
			model.advance_scene = true;
			model.current_note += 1;
		}
	} else {
		if app.elapsed_frames() % 10 == 0 {
			model.advance_scene = true;
			model.current_note += 1;
		}
	}

	if model.current_note == NOTES_PER_MEASURE {
		model.current_note = 0;
		model.current_measure += 1;
		if model.current_measure > scene.measures {
			model.current_measure = 0;
			if !scene.loop_ {
				model.next();
			}
		}
	}

	if model.advance_scene {
		let scene = &model.scenes[model.current_scene];
		for (value, target) in model.modifiers {
			model.values[target] += value;
		}
		for (i, &note) in scene.notes[model.current_note].iter().enumerate() {
			if note {
				model.values[i] = 0.0;
			}
		}
	}
	model.advance_scene = false;
}

fn view(app: &App, model: &Model, frame: Frame) {
	if app.elapsed_frames() % 60 != 0 {
		return;
	}
	let scene = &model.scenes[model.current_scene];
	let draw = app.draw();
	draw.background().color(BLACK);

	console::log_1(&"slow down".into());

	draw.text(&format!("{:?}", model.values)).x_y(-200.0, 200.0);

	if let Audio::Html(HtmlAudio {
		elements,
		current_element,
	}) = &model.audio
	{
		if elements[*current_element].ready_state() != 4 {
			draw.line()
				.gray(1.0)
				.points(Point2::new(-100.0, -100.0), Point2::new(100.0, 100.0));
			draw.to_frame(app, &frame).unwrap();
			return;
		}
	}

	for i in 0..CANVAS_WIDTH {
		for j in 0..CANVAS_HEIGHT {
			let noise2 = model.noisemap.get_value(i, j) as f32;
			let noise = model
				.noise_matrix
				.get([i as f64, j as f64, model.values[1] as f64]) as f32;
			let i = (i as f32 - (CANVAS_WIDTH / 2) as f32) * model.scaling as f32;
			let j = (j as f32 - (CANVAS_HEIGHT / 2) as f32) * model.scaling as f32;

			let mut gray = model.values[0] * noise2 * noise;
			let (i0, i1) = (i * 1.00 + noise2, (i + 20.0 * noise) + model.values[2]);
			let (j0, j1) = (
				j - 12.0 * noise.sin() - noise2 * 10.0,
				(j + 15.0 * noise.cos()) as f32,
			);

			if scene.boxes[0].inside(i0, j0) {
				gray = 1.0
			}

			draw.line()
				.gray(gray)
				.points(Point2::new(i0, j0), Point2::new(i1, j1));
		}
	}

	// let win = app.window_rect();
	// let fader_x = map_range(app.mouse.x, win.left(), win.right(), 0.0, 1.0);
	// let angle = deg_to_rad(360.0 / model.count as f32);

	// Write to the window frame.
	draw.to_frame(app, &frame).unwrap();
}

fn mouse_pressed(app: &App, model: &mut Model, _button: MouseButton) {
	let scene = &model.scenes[model.current_scene];
	model.play();

	if scene.boxes[0].inside(app.mouse.x, app.mouse.y) {
		model.current_scene += 1;
		model.current_note = 0;
		model.current_measure = 0;
		model.next();
	}
	// let future = wasm_bindgen_futures::JsFuture::from(promise);
	// future.await.unwrap();

	// if key == Key::S {
	//     app.main_window()
	//         .capture_frame(app.exe_name().unwrap() + ".png");
	// }
}

pub async fn run_app() {
	// Since ModelFn is not a closure we need this workaround to pass the calculated model
	thread_local!(static MODEL: RefCell<Option<Model>> = Default::default());

	let model = Model::new();
	MODEL.with(|m| m.borrow_mut().replace(model));

	app::Builder::new_async(|app| {
		Box::new(async move {
			create_window(app).await;
			MODEL.with(|m| m.borrow_mut().take().unwrap())
		})
	})
	.backends(Backends::PRIMARY | Backends::GL)
	.update(update)
	.run_async()
	.await;
}

async fn create_window(app: &App) {
	let device_desc = DeviceDescriptor {
		limits: Limits {
			max_texture_dimension_2d: 8192,
			..Limits::downlevel_webgl2_defaults()
		},
		..Default::default()
	};

	// let device_desc = DeviceDescriptor {
	//     limits: Limits {
	//         max_texture_dimension_1d: 8192,
	//         max_texture_dimension_2d: 8192,

	//         max_storage_buffers_per_shader_stage: 0,
	//         max_storage_textures_per_shader_stage: 0,
	//         max_dynamic_storage_buffers_per_pipeline_layout: 0,
	//         max_storage_buffer_binding_size: 0,
	//         max_vertex_buffer_array_stride: 255,

	//         // max_texture_dimension_1d: 2048,
	//         // max_texture_dimension_2d: 2048,
	//         max_texture_dimension_3d: 256,
	//         max_texture_array_layers: 256,
	//         max_bind_groups: 4,
	//         max_dynamic_uniform_buffers_per_pipeline_layout: 8,
	//         // max_dynamic_storage_buffers_per_pipeline_layout: 4,
	//         max_sampled_textures_per_shader_stage: 16,
	//         max_samplers_per_shader_stage: 16,
	//         // max_storage_buffers_per_shader_stage: 4,
	//         // max_storage_textures_per_shader_stage: 4,
	//         max_uniform_buffers_per_shader_stage: 12,
	//         max_uniform_buffer_binding_size: 16384,
	//         // max_storage_buffer_binding_size: 128 << 20,
	//         max_vertex_buffers: 8,
	//         max_vertex_attributes: 16,
	//         // max_vertex_buffer_array_stride: 2048,
	//         max_push_constant_size: 0,
	//         min_uniform_buffer_offset_alignment: 256,
	//         min_storage_buffer_offset_alignment: 256,
	//     },
	//     ..Default::default()
	// };
	let (width, height) = if WEB_TARGET {
		let window = web_sys::window().unwrap();
		(
			window.inner_width().unwrap().as_f64().unwrap() as u32,
			window.inner_height().unwrap().as_f64().unwrap() as u32,
		)
	} else {
		(
			(CANVAS_WIDTH * SCALING) as u32,
			(CANVAS_HEIGHT * SCALING) as u32,
		)
	};

	app.new_window()
		.device_descriptor(device_desc)
		.size(width as u32, height as u32)
		.title("Gender Theory")
		// .raw_event(raw_event)
		// .key_pressed(key_pressed)
		// .key_released(key_released)
		.mouse_pressed(mouse_pressed)
		// .mouse_moved(mouse_moved)
		// .mouse_released(mouse_released)
		// .mouse_wheel(mouse_wheel)
		// .touch(touch)
		.view(view)
		.build_async()
		.await
		.unwrap();

	// this doesn't seem to do anything but it's supposed to
	app.set_loop_mode(LoopMode::rate_fps(FPS));
}
