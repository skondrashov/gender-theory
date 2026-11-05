//! Where the player is and what they can see, kept in the browser's localStorage.

use crate::scenes::Unlocks;

const KEY: &str = "gender-theory/progress";

fn storage() -> Option<web_sys::Storage> {
	web_sys::window()?.local_storage().ok()?
}

/// (scene id, unlocks) saved by a previous visit.
pub fn load() -> Option<(String, Unlocks)> {
	let saved = storage()?.get_item(KEY).ok()??;
	let (scene, bits) = saved.split_once(' ')?;
	Some((scene.to_string(), Unlocks(bits.parse().ok()?)))
}

pub fn save(scene: &str, unlocks: Unlocks) {
	if let Some(storage) = storage() {
		let _ = storage.set_item(KEY, &format!("{scene} {}", unlocks.0));
	}
}

pub fn clear() {
	if let Some(storage) = storage() {
		let _ = storage.remove_item(KEY);
	}
}
