use {
	crate::scenes::*,
	nannou::{
		noise::{
			utils::{NoiseMap, NoiseMapBuilder, PlaneMapBuilder},
			OpenSimplex, Worley,
		},
		prelude::*,
		wgpu::{Backends, DeviceDescriptor, Limits},
	},
	std::cell::RefCell,
	web_sys::HtmlAudioElement,
};

pub const CANVAS_WIDTH: usize = 20;
pub const CANVAS_HEIGHT: usize = 10;
const SCALING: usize = 40;

struct Audio {
	elements: [HtmlAudioElement; 2],
	current_element: usize,
}

impl Model {
	fn play(&self) {
		let element = &self.audio.elements[self.audio.current_element];
		if element.ready_state() != 4 {
			return;
		}
		if element.paused() {
			element.set_loop(self.scenes[self.current_scene].loop_);
			let _ = element.play().unwrap();
		}
	}
	fn stop(&self) {
		self.audio.elements[self.audio.current_element]
			.pause()
			.unwrap();
	}
}

pub struct Model {
	pub noise_matrix: OpenSimplex, // [[[f32; CANVAS_HEIGHT]; CANVAS_WIDTH]; KEYFRAMES],
	pub noisemap: NoiseMap,
	pub scaling: usize,

	audio: Audio,
	scenes: Vec<&'static Scene>,
	queue_next: bool,
	current_scene: usize,
	current_measure: usize,
	last_whole: f64,
}

impl Model {
	pub fn new() -> Self {
		let mut scenes = vec![];
		scenes.extend_from_slice(for_against::SCENES);
		scenes.extend_from_slice(terra_firmament::SCENES);
		let audio = Audio {
			elements: [
				HtmlAudioElement::new_with_src(scenes[0].path).unwrap(),
				HtmlAudioElement::new_with_src(scenes[1].path).unwrap(),
			],
			current_element: 0,
		};
		Model {
			scaling: SCALING,
			noise_matrix: OpenSimplex::new(),
			noisemap: PlaneMapBuilder::new(&Worley::new())
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
}

fn update(_app: &App, model: &mut Model, _update: Update) {
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
				model.audio.elements[model.audio.current_element] =
					HtmlAudioElement::new_with_src(model.scenes[model.current_scene + 1].path)
						.unwrap();
				model.audio.current_element = if model.audio.current_element == 0 {
					1
				} else {
					0
				};
				model.play();
				last_whole = 0.0;
			}
		}
	}

	model.last_whole = last_whole;
}

fn view(app: &App, model: &Model, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);

	draw.text(&format!("Fps: {:?}", app.fps().round()))
		.x_y(-200.0, 220.0);
	let half = (model.last_whole as f32 % 0.5) * 2.0;
	let quarter = (half % 0.5) * 2.0;
	let eighth = (quarter % 0.5) * 2.0;
	let sixteenth = (eighth % 0.5) * 2.0;

	draw.text(&format!(
		"1({:.2}) 2({half:.2}) 4({quarter:.2}) 8({eighth:.2}) 16({sixteenth:.2})",
		model.last_whole
	))
	.x_y(-200.0, 180.0);

	let scene = &model.scenes[model.current_scene];
	if model.audio.elements[model.audio.current_element].ready_state() != 4 {
		draw.line()
			.gray(1.0)
			.points(Point2::new(-100.0, -100.0), Point2::new(100.0, 100.0));
		draw.to_frame(app, &frame).unwrap();
		return;
	}

	for i in 0..CANVAS_WIDTH {
		for j in 0..CANVAS_HEIGHT {
			(scene.render)(&draw, model, i, j, app.elapsed_frames(), model.last_whole);
		}
	}

	// let win = app.window_rect();
	// let fader_x = map_range(app.mouse.x, win.left(), win.right(), 0.0, 1.0);
	// let angle = deg_to_rad(360.0 / model.count as f32);

	// Write to the window frame.
	draw.to_frame(app, &frame).unwrap();
}

fn mouse_pressed(app: &App, model: &mut Model, _button: MouseButton) {
	model.play();

	if model.scenes[model.current_scene].boxes[0].inside(app.mouse.x, app.mouse.y) {
		model.queue_next = true;
	}
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
	let window = web_sys::window().unwrap();
	let (width, height) = (
		window.inner_width().unwrap().as_f64().unwrap() as u32,
		window.inner_height().unwrap().as_f64().unwrap() as u32,
	);

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
}
