use {
	async_std::task::block_on,
	model::{mouse_pressed, update, view, Model},
	nannou::{
		prelude::*,
		wgpu::{Backends, DeviceDescriptor, Limits},
	},
	std::cell::RefCell,
	wasm_bindgen::prelude::*,
};

mod model;
mod scenes;
// mod notes;

#[wasm_bindgen]
pub async fn main_web() {
	#[cfg(debug_assertions)]
	console_error_panic_hook::set_once();

	block_on(async {
		// Since ModelFn is not a closure we need this workaround to pass the calculated model
		thread_local!(static MODEL: RefCell<Option<Model>> = Default::default());

		let model = Model::new();
		MODEL.with(|m| m.borrow_mut().replace(model));

		app::Builder::new_async(|app| {
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
					.mouse_pressed(mouse_pressed)
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
