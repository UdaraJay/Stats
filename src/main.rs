mod config;
mod db;
mod handlers;
mod middleware;
mod models;
mod schema;
mod utils;

use crate::config::Config;
use crate::db::establish_connection_pool;
use crate::handlers::{collector, events, sessions};
use crate::utils::queue::{start_processing_events, EventsQueue};
use actix_files as fs;
use actix_web::{web, App, HttpServer};
use env_logger;
use log::info;
use middleware::cors::setup_cors;
use std::sync::{Arc, Mutex};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // let config: Config = Config::new();
    let config = Arc::new(Config::new());
    let address = format!("127.0.0.1:{}", config.service_port);
    let pool = establish_connection_pool();

    info!("STATS – A minimal analytics provider");
    info!("Starting server at http://{}", address);

    // Setup the background processing thread
    let events_queue = Arc::new(Mutex::new(EventsQueue::new(
        config.memory_limit_mb * 1024 * 1024,
    )));
    start_processing_events(web::Data::new(pool.clone()), events_queue.clone());

    HttpServer::new(move || {
        App::new()
            .wrap(setup_cors(&config.cors_domains))
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(events_queue.clone()))
            .route("/summary", web::get().to(events::summarize_events_json))
            .route("/collect", web::get().to(events::record_event))
            .route("/sessions", web::get().to(sessions::retrieve_sessions))
            .route("/events", web::get().to(events::retrieve_events))
            .route(
                "/summary/urls",
                web::get().to(events::retrieve_url_event_counts),
            )
            .route(
                "/summary/hourly",
                web::get().to(events::retrieve_hourly_summary),
            )
            .route("/stats.js", web::get().to(collector::serve_collector_js))
            .service(fs::Files::new("/", "src/frontend").index_file("index.html"))
    })
    .bind(address)?
    .run()
    .await
}
