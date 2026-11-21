//! Where the player is, what they can see and where they have been, kept in the browser's
//! localStorage: `<scene id> <unlock bits> [<scene id> <stage bits>]…` — the stages visited
//! of each scene, a bit per stage, for the hotspots whose names show once you have been there.

use {crate::scenes::Unlocks, std::collections::HashMap};

/// Scene id → a bit per stage the player has been in.
pub type Visited = HashMap<String, u64>;

const KEY: &str = "gender-theory/progress";

fn storage() -> Option<web_sys::Storage> {
	web_sys::window()?.local_storage().ok()?
}

/// (scene id, unlocks, stages visited) saved by a previous visit.
pub fn load() -> Option<(String, Unlocks, Visited)> {
	let saved = storage()?.get_item(KEY).ok()??;
	let mut words = saved.split(' ');
	let scene = words.next()?.to_string();
	let unlocks = Unlocks(words.next()?.parse().ok()?);
	let mut visited = Visited::new();
	while let (Some(id), Some(bits)) = (words.next(), words.next()) {
		if let Ok(bits) = bits.parse() {
			visited.insert(id.to_string(), bits);
		}
	}
	Some((scene, unlocks, visited))
}

pub fn save(scene: &str, unlocks: Unlocks, visited: &Visited) {
	if let Some(storage) = storage() {
		let mut text = format!("{scene} {}", unlocks.0);
		let mut been: Vec<_> = visited.iter().filter(|(_, bits)| **bits != 0).collect();
		been.sort();
		for (id, bits) in been {
			text.push_str(&format!(" {id} {bits}"));
		}
		let _ = storage.set_item(KEY, &text);
	}
}

pub fn clear() {
	if let Some(storage) = storage() {
		let _ = storage.remove_item(KEY);
	}
}
