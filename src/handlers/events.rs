use crate::config::Config;
use crate::db::DbPool;
use crate::models::{Event, NewEvent};
use actix_web::{web, HttpResponse, Responder, HttpRequest};
use chrono::Utc;
use diesel::prelude::*;
use log::{info, warn, error, debug};
use regex::Regex;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use ulid::Ulid;
use url::Url;

#[derive(Deserialize)]
pub struct EventQuery {
    url: String,
    referrer: Option<String>,
    name: String,
    collector_id: String,
}

pub async fn record_event(
    req: HttpRequest,
    config: web::Data<Arc<Config>>,
    events_queue: web::Data<Sender<NewEvent>>,
    item: web::Query<EventQuery>,
) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    
    let user_agent = req
        .headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    info!(
        "🔍 Event request received - IP: {}, UA: {}, URL: {}, Collector: {}, Event: {}",
        client_ip, user_agent, item.url, item.collector_id, item.name
    );

    // let localhost_regex =
    //     Regex::new(r"http://(127\.0\.0\.1|localhost|0\.0\.0\.0|\[::1\])(:\d+)?").unwrap();

    // // Block local requests in production
    // if !config.is_development && localhost_regex.is_match(&item.url) {
    //     warn!(
    //         "🚫 Blocked localhost request in production - IP: {}, URL: {}, Collector: {}",
    //         client_ip, item.url, item.collector_id
    //     );
    //     return HttpResponse::BadRequest().json("Localhost requests not allowed in production");
    // }

    // Validate collector_id format
    if item.collector_id.is_empty() {
        warn!(
            "🚫 Missing collector_id - IP: {}, URL: {}",
            client_ip, item.url
        );
        return HttpResponse::BadRequest().json("Missing collector_id");
    }

    // Validate URL format
    if item.url.is_empty() {
        warn!(
            "🚫 Missing URL - IP: {}, Collector: {}",
            client_ip, item.collector_id
        );
        return HttpResponse::BadRequest().json("Missing URL");
    }

    // Remove query parameters from the URL and trailing slashes
    let clean_url = match Url::parse(&item.url) {
        Ok(mut url) => {
            url.set_query(None);
            let mut url_str = url.to_string();
            // Remove trailing slash(es)
            url_str = url_str.trim_end_matches('/').to_string();
            debug!("🧹 Cleaned URL: {} -> {}", item.url, url_str);
            url_str
        }
        Err(e) => {
            warn!(
                "⚠️ URL parsing failed, using original - IP: {}, URL: {}, Error: {}",
                client_ip, item.url, e
            );
            // Also remove trailing slash(es) if URL parsing fails
            item.url.trim_end_matches('/').to_string()
        }
    };

    let event_id = Ulid::new().to_string();
    let new_event = NewEvent {
        id: event_id.clone(),
        url: clean_url.clone(),
        referrer: item.referrer.clone(),
        name: item.name.clone(),
        timestamp: Utc::now().naive_utc(),
        collector_id: item.collector_id.clone(),
    };

    debug!(
        "📤 Queueing event - ID: {}, URL: {}, Referrer: {:?}, Event: {}, Collector: {}",
        event_id, clean_url, item.referrer, item.name, item.collector_id
    );

    match events_queue.send(new_event).await {
        Ok(_) => {
            info!(
                "✅ Event queued successfully - ID: {}, IP: {}, URL: {}, Event: {}",
                event_id, client_ip, clean_url, item.name
            );
            HttpResponse::Ok().json("Event recorded successfully")
        }
        Err(e) => {
            error!(
                "❌ Failed to queue event - ID: {}, IP: {}, URL: {}, Error: {}",
                event_id, client_ip, clean_url, e
            );
            HttpResponse::ServiceUnavailable().json("Failed to process event")
        }
    }
}

pub async fn retrieve_events(pool: web::Data<DbPool>) -> impl Responder {
    info!("📊 Retrieving events from database");
    use crate::schema::events::dsl::*;

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for events retrieval");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for events retrieval: {}", e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

    match events.load::<Event>(&mut conn) {
        Ok(events_list) => {
            info!("✅ Successfully retrieved {} events", events_list.len());
            HttpResponse::Ok().json(events_list)
        }
        Err(e) => {
            error!("❌ Failed to retrieve events from database: {}", e);
            HttpResponse::InternalServerError().json("Error retrieving events")
        }
    }
}
