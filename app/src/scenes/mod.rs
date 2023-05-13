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
	y0: f32,
	x1: f32,
	y1: f32,
}

fn scale_coordinates(model: &Model, i: usize, j: usize) -> (f32, f32) {
	(
		(i as f32 - (CANVAS_WIDTH as f32 / 2.0) as f32) * model.scaling as f32,
		(j as f32 - (CANVAS_HEIGHT as f32 / 2.0) as f32) * model.scaling as f32,
	)
}

fn u8_to_f32(n: u8) -> f32 {
	n as f32 / 255.0
}

pub fn get_modifiers(model: &Model) -> [f32; 8] {
	let half = (model.last_whole as f32 % 0.5) * 2.0;
	let quarter = (half % 0.5) * 2.0;
	let eighth = (quarter % 0.5) * 2.0;
	let sixteenth = (eighth % 0.5) * 2.0;

	let mut fft = [0; 16];
	model.audio.analyser.get_byte_frequency_data(&mut fft);

	let bass = (u8_to_f32(fft[0]) + u8_to_f32(fft[1])) as f32 / 2.0;
	let mid = (u8_to_f32(fft[2])
		+ u8_to_f32(fft[3])
		+ u8_to_f32(fft[4])
		+ u8_to_f32(fft[5])
		+ u8_to_f32(fft[6])
		+ u8_to_f32(fft[7])) as f32
		/ 6.0;
	let treble = (u8_to_f32(fft[8])
		+ u8_to_f32(fft[9])
		+ u8_to_f32(fft[10])
		+ u8_to_f32(fft[11])
		+ u8_to_f32(fft[12])
		+ u8_to_f32(fft[13])
		+ u8_to_f32(fft[14])
		+ u8_to_f32(fft[15])) as f32
		/ 8.0;

	[
		model.last_whole as f32,
		half,
		quarter,
		eighth,
		sixteenth,
		bass,
		mid,
		treble,
	]
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

	pub render: &'static dyn Fn(&Draw, &Model, usize, usize, f32, [f32; 8]),
	pub boxes: &'static [Area],
}
