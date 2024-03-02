use crate::config::Config;
use crate::db::DbPool;
use crate::models::{Event, NewEvent};
use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use diesel::prelude::*;
use log::info;
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
    config: web::Data<Arc<Config>>,
    events_queue: web::Data<Sender<NewEvent>>,
    item: web::Query<EventQuery>,
) -> impl Responder {
    let localhost_regex =
        Regex::new(r"http://(127\.0\.0\.1|localhost|0\.0\.0\.0|\[::1\])(:\d+)?").unwrap();

    // Block local requests in production
    // TODO: i don't think cors is taking care of this because
    // the origin is not available in localhost?
    if !config.is_development && localhost_regex.is_match(&item.url) {
        return HttpResponse::BadRequest().finish();
    }

    // Remove query parameters from the URL and trailing slashes
    let clean_url = match Url::parse(&item.url) {
        Ok(mut url) => {
            url.set_query(None);
            let mut url_str = url.to_string();
            // Remove trailing slash(es)
            url_str = url_str.trim_end_matches('/').to_string();
            url_str
        }
        Err(_) => {
            // Also remove trailing slash(es) if URL parsing fails
            item.url.trim_end_matches('/').to_string()
        }
    };

    let new_event = NewEvent {
        id: Ulid::new().to_string(),
        url: clean_url,
        referrer: item.referrer.clone(),
        name: item.name.clone(),
        timestamp: Utc::now().naive_utc(),
        collector_id: item.collector_id.clone(),
    };

    match events_queue.send(new_event).await {
        Ok(_) => HttpResponse::Ok().json("Event recorded successfully"),
        Err(_) => {
            eprintln!("Failed to send event to the processing channel.");
            HttpResponse::ServiceUnavailable().json("Failed to process event")
        }
    }
}

pub async fn retrieve_events(pool: web::Data<DbPool>) -> impl Responder {
    info!("Retrieving events");
    use crate::schema::events::dsl::*;
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    match events.load::<Event>(&mut conn) {
        Ok(events_list) => HttpResponse::Ok().json(events_list),
        Err(_) => HttpResponse::InternalServerError().json("Error retrieving events"),
    }
}
