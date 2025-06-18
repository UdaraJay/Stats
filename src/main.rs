mod config;
mod db;
mod handlers;
mod middleware;
mod models;
mod schema;
mod utils;

use crate::config::Config;
use crate::db::establish_connection_pool;
use crate::handlers::{collector, events, sessions, summary};
use crate::models::NewEvent;
use crate::utils::queue::process_events_async;
use actix_files as fs;
use actix_web::{web, App, HttpResponse, HttpServer, HttpRequest};
use env_logger;
use log::{info, warn, error, debug};
use middleware::cors::setup_cors;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

// Scheduler tasks
async fn HourlyScheduler() {
    info!("⏰ Hourly scheduler started");
    loop {
        // Task to be executed every 1 hour
        info!("⏰ Scheduler executing hourly task...");
        // Replace the following line with your actual task
        // Perform your task here...

        // Sleep for 1 hour
        debug!("⏰ Scheduler sleeping for 1 hour");
        sleep(Duration::from_secs(3600)).await;
    }
}

// Default service handler with logging
async fn default_handler(req: HttpRequest) -> HttpResponse {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let path = req.path();
    let method = req.method();
    
    warn!("🚫 Unhandled request - IP: {}, Method: {}, Path: {}", client_ip, method, path);
    HttpResponse::NoContent().finish()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let config = Arc::new(Config::new());
    let address = format!("0.0.0.0:{}", config.service_port);
    let pool = establish_connection_pool();

    info!("🚀 Stats Analytics Server Starting Up");
    info!("📊 Server Configuration:");
    info!("  - Address: {}", address);
    info!("  - App URL: {}", config.app_url);
    info!("  - Database: {}", config.database_url);
    info!("  - CORS Domains: {:?}", config.cors_domains);
    info!("  - Processing Batch Size: {}", config.processing_batch_size);
    info!("  - Development Mode: {}", config.is_development);

    // Test database connection
    match pool.get() {
        Ok(_) => {
            info!("✅ Database connection pool established successfully");
        }
        Err(e) => {
            error!("❌ Failed to establish database connection pool: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Database connection failed"));
        }
    }

    info!("🔄 Starting background scheduler...");
    // Start scheduler
    let scheduler = tokio::spawn(async {
        HourlyScheduler().await;
    });

    info!("📬 Setting up event processing queue...");
    // Setup the background processing queue
    let (events_queue, rx) = mpsc::channel::<NewEvent>(500);
    let db_pool = pool.clone();
    tokio::spawn(async move {
        process_events_async(rx, db_pool).await;
    });

    info!("🌐 Registering HTTP routes...");
    // Start the HTTP server
    // serves the API and the static dashboard in the `ui` directory
    HttpServer::new(move || {
        info!("🔧 Creating new app instance");
        App::new()
            .wrap(setup_cors(&config.cors_domains))
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(events_queue.clone()))
            // API routes
            .route("/collect", web::get().to(events::record_event))
            .route("/create-collector", web::post().to(collector::post_collector))
            .route("/sessions", web::get().to(sessions::retrieve_sessions))
            .route("/sessions/map", web::get().to(sessions::map))
            .route("/summary", web::get().to(summary::events))
            .route("/summary/urls", web::get().to(summary::urls))
            .route("/summary/hourly", web::get().to(summary::hourly))
            .route("/summary/weekly", web::get().to(summary::weekly))
            .route("/summary/fiveminutes", web::get().to(summary::five_minutes))
            .route("/summary/browsers", web::get().to(summary::browsers))
            .route("/summary/osbrowsers", web::get().to(summary::os_browsers))
            .route("/summary/referrers", web::get().to(summary::referrers))
            .route("/summary/percentages", web::get().to(summary::percentages))
            .route("/stats.js", web::get().to(collector::serve_collector_js))
            // Static file serving
            .service(fs::Files::new("/", "ui").index_file("index.html"))
            .default_service(web::route().to(default_handler))
    })
    .bind(&address)?
    .run()
    .await
}
