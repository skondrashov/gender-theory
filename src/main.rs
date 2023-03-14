use {async_std::task::block_on, model::run_app};

mod model;
mod notes;

// native app entry point
fn main() {
	block_on(async {
		run_app().await;
	});
}
