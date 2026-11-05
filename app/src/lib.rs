use {
	async_std::task::block_on,
	model::{key_pressed, mouse_moved, mouse_pressed, mouse_released, update, view, Model},
	nannou::{
		prelude::*,
		wgpu::{Backends, DeviceDescriptor, Limits},
	},
	std::cell::RefCell,
	wasm_bindgen::prelude::*,
};

mod audio;
mod model;
mod progress;
mod scenes;
// mod notes;

#[wasm_bindgen]
pub async fn main_web() {
	console_error_panic_hook::set_once();

	block_on(async {
		// Since ModelFn is not a closure we need this workaround to pass the calculated model
		thread_local!(static MODEL: RefCell<Option<Model>> = Default::default());

		// URL flags: ?debug shows the overlay, ?msaa=N sets window multisampling (default 4),
		// ?scene=<id> starts on that scene, ?reset forgets saved progress,
		// ?unlock=lines+rotation grants those unlocks.
		let search = web_sys::window()
			.and_then(|w| w.location().search().ok())
			.unwrap_or_default();
		let debug = query_param(&search, "debug").is_some();
		let msaa = query_param(&search, "msaa")
			.and_then(|v| v.parse::<u32>().ok())
			.unwrap_or(4);
		let start = query_param(&search, "scene").filter(|id| !id.is_empty());
		let reset = query_param(&search, "reset").is_some();
		let unlock = query_param(&search, "unlock");
		let model = Model::new(debug, start, reset, unlock);
		MODEL.with(|m| m.borrow_mut().replace(model));

		app::Builder::new_async(move |app| {
			Box::new(async move {
				let device_desc = DeviceDescriptor {
					limits: Limits {
						max_texture_dimension_2d: 8192,
						..Limits::downlevel_webgl2_defaults()
					},
					..Default::default()
				};

				let window = web_sys::window().unwrap();
				let (width, height) = (
					window.inner_width().unwrap().as_f64().unwrap() as u32,
					window.inner_height().unwrap().as_f64().unwrap() as u32,
				);

				app.new_window()
					.device_descriptor(device_desc)
					.size(width as u32, height as u32)
					.msaa_samples(msaa)
					.mouse_pressed(mouse_pressed)
					.mouse_released(mouse_released)
					.mouse_moved(mouse_moved)
					.key_pressed(key_pressed)
					.view(view)
					.build_async()
					.await
					.unwrap();
				MODEL.with(|m| m.borrow_mut().take().unwrap())
			})
		})
		.backends(Backends::PRIMARY | Backends::GL)
		.update(update)
		.run_async()
		.await;
	});
}

fn query_param(search: &str, key: &str) -> Option<String> {
	search
		.trim_start_matches('?')
		.split('&')
		.find_map(|kv| {
			let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
			(k == key).then(|| v.to_string())
		})
}
