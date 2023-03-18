use {
	crate::model::{Model, CANVAS_HEIGHT, CANVAS_WIDTH},
	nannou::Draw,
};

// pub mod being_useless;
pub mod for_against;
// pub mod porcelain_piss;
pub mod terra_firmament;

pub struct Area {
	x0: f32,
	x1: f32,
	y0: f32,
	y1: f32,
}

fn scale_coordinates(model: &Model, i: usize, j: usize) -> (f32, f32) {
	(
		(i as f32 - (CANVAS_WIDTH as f32 / 2.0) as f32) * model.scaling as f32,
		(j as f32 - (CANVAS_HEIGHT as f32 / 2.0) as f32) * model.scaling as f32,
	)
}

impl Area {
	pub fn inside(&self, x: f32, y: f32) -> bool {
		self.x0 < x && x < self.x1 && self.y0 < y && y < self.y1
	}
}

pub struct Scene {
	pub loop_: bool,
	pub path: &'static str,
	pub measures: usize,

	pub render: &'static dyn Fn(&Draw, &Model, usize, usize, u64, f64),
	pub boxes: [Area; 2],
}
