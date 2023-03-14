use crate::model::{EFFECTS, NOTES_PER_MEASURE};

#[allow(non_upper_case_globals)]
const o: bool = false;
const X: bool = true;

pub const NOTES: &[[[bool; EFFECTS]; NOTES_PER_MEASURE]] = &[
	[
		[X, X, o, X], //
		[o, o, o, X],
		[o, o, o, X],
		[o, o, o, X],
		[o, o, X, X], //
		[o, o, o, X],
		[o, o, o, X],
		[o, o, o, X],
		[o, X, o, X], //
		[o, o, o, X],
		[o, o, X, X],
		[o, o, o, X],
		[o, o, o, X], //
		[o, o, o, X],
		[o, o, o, X],
		[o, o, o, X],
	],
	[
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
		[o, o, o, o],
	],
];
