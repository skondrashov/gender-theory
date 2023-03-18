use {async_std::task::block_on, model::run_app, wasm_bindgen::prelude::*};

mod model;
mod scenes;
// mod notes;

// web app entry point
#[wasm_bindgen]
pub async fn main_web() {
	#[cfg(debug_assertions)]
	console_error_panic_hook::set_once();

	block_on(async {
		run_app().await;
	});
}
