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
use crate::models::NewEvent;
use crate::utils::queue::process_events_async;
use actix_files as fs;
use actix_web::{web, App, HttpServer};
use env_logger;
use log::info;
use middleware::cors::setup_cors;
use std::sync::Arc;
use tokio::sync::mpsc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let config = Arc::new(Config::new());
    let address = format!("127.0.0.1:{}", config.service_port);
    let pool = establish_connection_pool();

    info!("Stats analytics");
    info!("Starting server at `http://{}", address);

    // Setup the MPSC channel
    let (events_queue, rx) = mpsc::channel::<NewEvent>(500);

    // Start the event processing task
    let db_pool = pool.clone();
    tokio::spawn(async move {
        process_events_async(rx, db_pool).await;
    });

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
