use {
	crate::scenes::{Area, Scene, *},
	nannou::{noise::NoiseFn, prelude::*},
};

pub const SCENES: &[&Scene] = &[
	&Scene {
		path: "/fa/1.mp3",
		loop_: true,
		measures: 14,
		render: &|draw, model, i, j, time, whole| {
			let half = (whole as f32 % 0.5) * 2.0;
			let quarter = (half % 0.5) * 2.0;
			let eighth = (quarter % 0.5) * 2.0;
			let sixteenth = (eighth % 0.5) * 2.0;

			let noise = model.noisemap.get_value(i, j) as f32;
			let noise2 = model
				.noise_matrix
				.get([i as f64, j as f64, time as f64 / 1000.0]) as f32;

			let (i, j) = scale_coordinates(model, i, j);
			let mut gray = half * noise * noise2 * 3.0
				+ 0.2 * (quarter - 1.0).abs()
				+ 0.7 * noise2 * noise2 * noise2;
			let (i0, i1) = (i * 1.00 + noise2, (i + 20.0 * noise) + eighth);
			let (j0, j1) = (
				j - 12.0 * quarter.sin() - noise2 * 10.0,
				(j + 3.0 * noise.cos() * sixteenth * eighth * eighth.sin()),
			);

			if (Area {
				x0: -100.0,
				y0: -100.0,
				x1: 100.0,
				y1: 100.0,
			})
			.inside(i0, j0)
			{
				gray = 1.0
			}

			draw.line()
				.gray(gray)
				.points(Point2::new(i0, j0), Point2::new(i1, j1));
		},
		boxes: [
			Area {
				x0: -80.0,
				y0: -80.0,
				x1: 80.0,
				y1: 80.0,
			},
			Area {
				x0: 0.0,
				y0: 0.0,
				x1: 0.0,
				y1: 0.0,
			},
		],
	},
	&Scene {
		path: "/fa/2.mp3",
		loop_: true,
		measures: 8,
		render: &|draw, model, i, j, time, whole| {
			let half = (whole as f32 % 0.5) * 2.0;
			let quarter = (half % 0.5) * 2.0;
			let eighth = (quarter % 0.5) * 2.0;
			let _sixteenth = (eighth % 0.5) * 2.0;

			let noise = model.noisemap.get_value(i, j) as f32;
			let noise2 = model
				.noise_matrix
				.get([i as f64, j as f64, time as f64 / 100.0]) as f32;

			let time = time as f32 / 100.0;
			let (i, j) = scale_coordinates(model, i, j);
			let mut gray = half * noise * noise2 * 3.0 + 0.3 * noise2 + 0.3 - whole as f32 * 0.5
				+ 0.1 * (time / 10.0).sin();
			let (i0, i1) = (
				i * 1.00 + 5.0 * noise2 * time.cos(),
				(i + 20.0 * noise) + eighth * time.cos() * 20.0 * noise,
			);
			let (j0, j1) = (
				j - 12.0 * (quarter * 3.0).sin() - noise2 * 10.0 + 5.0 * time.sin(),
				j + 3.0 * noise.cos() * eighth * (eighth * 2.0).sin() + 30.0 * noise2 * time.sin(),
			);

			if (Area {
				x0: -100.0,
				y0: -100.0,
				x1: 100.0,
				y1: 100.0,
			})
			.inside(i0, j0)
			{
				gray = 0.0
			}

			draw.line()
				.gray(gray)
				.points(Point2::new(i0, j0), Point2::new(i1, j1));
		},
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
