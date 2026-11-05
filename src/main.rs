use actix_files::Files;
use actix_web::{middleware::Logger, web, App, HttpResponse, HttpServer};

/// The editor saves the album here. Dev only: the server binds 127.0.0.1.
async fn save_album(body: web::Bytes) -> HttpResponse {
	if serde_json::from_slice::<serde_json::Value>(&body).is_err() {
		return HttpResponse::BadRequest().body("not JSON");
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
			.wrap(Logger::default())
	})
	.bind(("127.0.0.1", 8080))?
	.run()
	.await
}
