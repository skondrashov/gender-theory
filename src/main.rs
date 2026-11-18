use actix_files::Files;
use actix_web::{
	middleware::{DefaultHeaders, Logger},
	web, App, HttpRequest, HttpResponse, HttpServer,
};

/// A fingerprint of the album file (32-bit FNV-1a, hex), the same as the editor computes
/// over the text it read or wrote.
fn fingerprint(bytes: &[u8]) -> String {
	let mut h: u32 = 0x811c9dc5;
	for &b in bytes {
		h = (h ^ b as u32).wrapping_mul(16777619);
	}
	format!("{h:08x}")
}

/// The editor saves the album here. Dev only: the server binds 127.0.0.1. A save that says
/// which file it's based on (`X-Album-Base`) is refused with 409 if the file on disk is
/// another — a tab that loaded before a later save (or a git checkout) shouldn't overwrite
/// it without meaning to.
async fn save_album(req: HttpRequest, body: web::Bytes) -> HttpResponse {
	if serde_json::from_slice::<serde_json::Value>(&body).is_err() {
		return HttpResponse::BadRequest().body("not JSON");
	}
	if let Some(base) = req.headers().get("x-album-base").and_then(|v| v.to_str().ok()) {
		let current = std::fs::read("./static/album.json").map(|bytes| fingerprint(&bytes)).unwrap_or_default();
		if current != base {
			return HttpResponse::Conflict().body("album.json on disk is not the one this editor loaded");
		}
	}
	match std::fs::write("./static/album.json", &body) {
		Ok(()) => HttpResponse::Ok().body("saved"),
		Err(error) => HttpResponse::InternalServerError().body(error.to_string()),
	}
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

	log::info!("starting HTTP server at http://localhost:8080");

	HttpServer::new(|| {
		App::new()
			.app_data(web::PayloadConfig::new(16 << 20))
			.route("/album.json", web::put().to(save_album))
			.service(Files::new("/app", "./app/pkg").show_files_listing())
			.service(Files::new("/", "./static").index_file("index.html"))
			// Everything revalidates every load (ETags make that cheap): a rebuilt wasm or a
			// saved album must never be served stale from the browser's cache.
			.wrap(DefaultHeaders::new().add(("Cache-Control", "no-cache")))
			.wrap(Logger::default())
	})
	.bind(("127.0.0.1", 8080))?
	.run()
	.await
}
