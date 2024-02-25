use crate::db::DbPool;
use crate::models::{Event, NewEvent};
use crate::utils::queue::EventsQueue;
use actix_web::{web, HttpResponse, Responder};
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Date, Integer, Timestamp};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};
use ulid::Ulid; // Make sure to include serde_json for JSON handling

use diesel::deserialize::FromSqlRow;
use diesel::row::Row;
use diesel::sql_types::Text;
use diesel::QueryResult;

#[derive(QueryableByName, Debug, Serialize)]
pub struct EventCounts {
    #[sql_type = "BigInt"]
    pub sessionsInLastTwentyFourHours: i64,
    #[sql_type = "BigInt"]
    pub eventsInLastTwentyFourHours: i64,
    #[sql_type = "BigInt"]
    pub eventsInLastHour: i64,
    #[sql_type = "BigInt"]
    pub eventsInLastFiveMinutes: i64,
}

#[derive(Deserialize)]
pub struct EventQuery {
    url: String,
    name: String,
    collector_id: String,
}

#[derive(QueryableByName, Debug)]
struct DailyEventCount {
    #[diesel(sql_type = Date)]
    day: NaiveDate,
    #[diesel(sql_type = Integer)]
    count: i32,
}

pub async fn summarize_events(pool: web::Data<DbPool>) -> impl Responder {
    info!("Visualizing events for the last 30 days");

    let mut conn = pool.get().expect("couldn't get db connection from pool");
    let today = Utc::now().naive_utc();
    let thirty_days_ago = today - Duration::days(30);

    let query = diesel::sql_query(
        "SELECT date(timestamp) as day, COUNT(*) as count \
         FROM events \
         WHERE timestamp >= ? \
         GROUP BY date(timestamp) \
         ORDER BY day ASC",
    )
    .bind::<Timestamp, _>(thirty_days_ago);

    match query.load::<DailyEventCount>(&mut conn) {
        Ok(counts) => {
            let mut visualization = String::new();
            for entry in counts {
                // let bar = "]".repeat(entry.count as usize);
                let line = format!(
                    "{} > {}\n",
                    entry.day.format("%Y-%m-%d").to_string(),
                    entry.count,
                );
                visualization.push_str(&line);
            }
            HttpResponse::Ok()
                .content_type("text/plain")
                .body(visualization)
        }
        Err(e) => {
            HttpResponse::InternalServerError().json(format!("Error visualizing events: {:?}", e))
        }
    }
}

pub async fn summarize_events_json(pool: web::Data<DbPool>) -> impl Responder {
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let query = diesel::sql_query(
        "SELECT \
        (SELECT COUNT(*) FROM collectors WHERE timestamp >= datetime('now', '-24 hours')) AS sessionsInLastTwentyFourHours, \
        (SELECT COUNT(*) FROM events WHERE timestamp >= datetime('now', '-24 hours')) AS eventsInLastTwentyFourHours, \
        (SELECT COUNT(*) FROM events WHERE timestamp >= datetime('now', '-5 minutes')) AS eventsInLastFiveMinutes, \
        (SELECT COUNT(*) FROM events WHERE timestamp >= datetime('now', '-1 hour')) AS eventsInLastHour"
    );

    match query.load::<EventCounts>(&mut conn) {
        Ok(counts) => {
            // Assuming there's only one row of counts, since the query aggregates data.
            if let Some(counts) = counts.into_iter().next() {
                HttpResponse::Ok().json(json!({
                    "eventsInLastHour": counts.eventsInLastHour,
                    "eventsInLastFiveMinutes": counts.eventsInLastFiveMinutes,
                    "eventsInLastTwentyFourHours": counts.eventsInLastTwentyFourHours,
                    "sessionsInLastTwentyFourHours": counts.sessionsInLastTwentyFourHours,
                }))
            } else {
                HttpResponse::Ok().json(json!({
                    "error": "No data available"
                }))
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": format!("Error querying events: {:?}", e)
        })),
    }
}

pub async fn record_event(
    events_queue: web::Data<Arc<Mutex<EventsQueue>>>,
    item: web::Query<EventQuery>,
) -> impl Responder {
    info!("Record: Event {}", item.name);

    // Construct a NewEvent object
    let new_event = NewEvent {
        id: Ulid::new().to_string(),
        url: item.url.clone(),
        name: item.name.clone(),
        timestamp: Utc::now().naive_utc(),
        collector_id: item.collector_id.clone(),
    };

    // Attempt to push the new event onto the analytics queue
    match events_queue.lock().unwrap().push(new_event) {
        Ok(_) => HttpResponse::Ok().json("Event recorded successfully"),
        Err(e) => {
            warn!("Failed to record event: {}", e);
            HttpResponse::InternalServerError().json("Error recording event")
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

// Define a struct to hold the summary data
#[derive(QueryableByName, Serialize)]
struct HourlyEventSummary {
    #[sql_type = "Timestamp"]
    hour: NaiveDateTime,
    #[sql_type = "Integer"]
    count: i32,
}

// Handler to retrieve the summarized events
pub async fn retrieve_hourly_summary(pool: web::Data<DbPool>) -> impl Responder {
    let start_time = Utc::now().naive_utc() - Duration::days(1);
    let mut conn = match pool.get() {
        Ok(conn) => conn,
        Err(_) => return HttpResponse::ServiceUnavailable().json("Could not get DB connection"),
    };

    let sql = "
        SELECT strftime('%Y-%m-%d %H:00:00', timestamp) AS hour, COUNT(*) AS count
        FROM events
        WHERE timestamp > ?
        GROUP BY strftime('%Y-%m-%d %H', timestamp)
        ORDER BY hour ASC;
    ";

    let results: Result<Vec<HourlyEventSummary>, diesel::result::Error> = diesel::sql_query(sql)
        .bind::<Timestamp, _>(start_time)
        .load(&mut conn);

    match results {
        Ok(summary) => HttpResponse::Ok().json(summary),
        Err(e) => {
            eprintln!("Database query failed: {:?}", e); // Log the error to stderr
            HttpResponse::InternalServerError().json(format!("Database error: {:?}", e))
        }
    }
}

#[derive(Serialize, Deserialize, QueryableByName)]
pub struct UrlEventCount {
    #[sql_type = "Text"]
    pub url: String,
    #[sql_type = "BigInt"]
    pub count: i64,
}

pub async fn retrieve_url_event_counts(pool: web::Data<DbPool>) -> impl Responder {
    let start_time = Utc::now().naive_utc() - Duration::days(1);
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let sql = "
        SELECT url, COUNT(*) AS count
        FROM events
        WHERE timestamp > ?
        GROUP BY url
        ORDER BY count DESC;
    ";

    let results: Result<Vec<UrlEventCount>, diesel::result::Error> = diesel::sql_query(sql)
        .bind::<Timestamp, _>(start_time)
        .load(&mut conn);

    match results {
        Ok(url_counts) => HttpResponse::Ok().json(url_counts),
        Err(e) => {
            eprintln!("Database query failed: {:?}", e); // Log the error to stderr
            HttpResponse::InternalServerError().json(format!("Database error: {:?}", e))
        }
    }
}
