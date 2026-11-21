//! The exits' and the stages' names: `words::random_words` shuffled once per game. The
//! shuffle's seed is kept in localStorage beside the progress, so names hold through saves,
//! transitions and reloads until they're reset (the editor's "reset names", or the game's
//! `?reset`). Room `r`'s exit `k` is word `r × 4 + k` of the shuffle, counted from its start;
//! room `r`'s stage `s` is word `r × 16 + s` counted from its end — so no two share a word
//! (owner, 2026-11-21: "it's the stages that i want to have random names, all of them").

use {
	crate::words::random_words,
	std::cell::{Cell, RefCell},
};

const KEY: &str = "gender-theory/names";
/// Exits a room may name: "name 1" … "name 4".
pub const PER_ROOM: usize = 4;
/// Stages a room may name, from the other end of the shuffle.
pub const STAGES_PER_ROOM: usize = 16;

thread_local! {
	static SEED: Cell<Option<u64>> = Cell::new(None);
	/// (seed, the shuffle) last built.
	static ORDER: RefCell<Option<(u64, Vec<u16>)>> = RefCell::new(None);
}

fn storage() -> Option<web_sys::Storage> {
	web_sys::window()?.local_storage().ok()?
}

fn fresh() -> u64 {
	let r = (js_sys::Math::random() * 9007199254740992.0) as u64;
	r | 1
}

fn store(seed: u64) {
	if let Some(storage) = storage() {
		let _ = storage.set_item(KEY, &seed.to_string());
	}
}

/// This game's seed: the saved one, or a new one saved now.
pub fn seed() -> u64 {
	SEED.with(|s| {
		if let Some(seed) = s.get() {
			return seed;
		}
		let saved = storage()
			.and_then(|storage| storage.get_item(KEY).ok().flatten())
			.and_then(|text| text.parse::<u64>().ok());
		let seed = saved.unwrap_or_else(|| {
			let seed = fresh();
			store(seed);
			seed
		});
		s.set(Some(seed));
		seed
	})
}

/// New names from now on (and saved).
pub fn reset() {
	let seed = fresh();
	store(seed);
	SEED.with(|s| s.set(Some(seed)));
}

/// Forget the names; the next game draws new ones.
pub fn clear() {
	if let Some(storage) = storage() {
		let _ = storage.remove_item(KEY);
	}
	SEED.with(|s| s.set(None));
}

/// Word `n` of this game's shuffle.
fn word(n: usize) -> &'static str {
	let seed = seed();
	ORDER.with(|order| {
		let mut order = order.borrow_mut();
		if order.as_ref().map_or(true, |(s, _)| *s != seed) {
			*order = Some((seed, shuffle(seed, random_words.len())));
		}
		let (_, order) = order.as_ref().unwrap();
		random_words[order[n % order.len()] as usize]
	})
}

/// The k-th exit name of room `room` (rooms are the album's scenes, in order).
pub fn exit(room: usize, k: usize) -> &'static str {
	word(room * PER_ROOM + k)
}

/// The name of stage `stage` of room `room`: from the far end of the shuffle, clear of the exits.
pub fn stage(room: usize, stage: usize) -> &'static str {
	word(random_words.len() - 1 - (room * STAGES_PER_ROOM + stage) % random_words.len())
}

/// A hotspot's name as shown: "name N" is the room's N-th word; anything else is itself.
pub fn resolve(name: &str, room: usize) -> String {
	let lower = name.trim().to_lowercase();
	if let Some(n) = lower.strip_prefix("name ").and_then(|n| n.trim().parse::<usize>().ok()) {
		if n >= 1 {
			return exit(room, n - 1).to_string();
		}
	}
	name.trim().to_string()
}

/// Fisher–Yates over 0..n with xorshift64* from `seed`.
fn shuffle(seed: u64, n: usize) -> Vec<u16> {
	let mut state = seed.max(1);
	let mut next = move || {
		state ^= state >> 12;
		state ^= state << 25;
		state ^= state >> 27;
		state.wrapping_mul(0x2545F4914F6CDD1D)
	};
	let mut order: Vec<u16> = (0..n as u16).collect();
	for i in (1..n).rev() {
		let j = (next() % (i as u64 + 1)) as usize;
		order.swap(i, j);
	}
	order
}
