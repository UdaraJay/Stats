use crate::db::DbPool;
use actix_web::{web, HttpResponse, Responder};
use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Text, Timestamp};
use serde::{Deserialize, Serialize};

use serde_json::json;
#[derive(QueryableByName, Debug, Serialize)]
pub struct EventCounts {
    #[diesel(sql_type = BigInt)]
    pub sessions_in_last_twenty_four_hours: i64,
    #[diesel(sql_type = BigInt)]
    pub events_in_last_twenty_four_hours: i64,
    #[diesel(sql_type = BigInt)]
    pub events_in_last_hour: i64,
    #[diesel(sql_type = BigInt)]
    pub events_in_last_five_minutes: i64,
}

#[derive(QueryableByName, Debug, Serialize)]
pub struct FiveMinuteEventSummary {
    #[diesel(sql_type = Text)]
    pub interval: String,
    #[diesel(sql_type = BigInt)]
    pub count: i64,
}

pub async fn five_minutes(pool: web::Data<DbPool>) -> impl Responder {
    let start_time = Utc::now().naive_utc() - Duration::days(1);
    let mut conn = match pool.get() {
        Ok(conn) => conn,
        Err(_) => return HttpResponse::ServiceUnavailable().json("Could not get DB connection"),
    };

    let sql = "
        SELECT strftime('%Y-%m-%d %H:%M:00', timestamp) AS interval, COUNT(*) AS count
        FROM events
        WHERE timestamp > ?
        GROUP BY strftime('%Y-%m-%d %H:%M', timestamp)
        ORDER BY interval ASC;
    ";

    let results: Result<Vec<FiveMinuteEventSummary>, diesel::result::Error> =
        diesel::sql_query(sql)
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

pub async fn events(pool: web::Data<DbPool>) -> impl Responder {
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let query = diesel::sql_query(
        "SELECT \
        (SELECT COUNT(*) FROM collectors WHERE timestamp >= datetime('now', '-24 hours')) AS sessions_in_last_twenty_four_hours, \
        (SELECT COUNT(*) FROM events WHERE timestamp >= datetime('now', '-24 hours')) AS events_in_last_twenty_four_hours, \
        (SELECT COUNT(*) FROM events WHERE timestamp >= datetime('now', '-5 minutes')) AS events_in_last_five_minutes, \
        (SELECT COUNT(*) FROM events WHERE timestamp >= datetime('now', '-1 hour')) AS events_in_last_hour"
    );

    match query.load::<EventCounts>(&mut conn) {
        Ok(counts) => {
            if let Some(counts) = counts.into_iter().next() {
                HttpResponse::Ok().json(json!({
                    "events_in_last_hour": counts.events_in_last_hour,
                    "events_in_last_five_minutes": counts.events_in_last_five_minutes,
                    "events_in_last_twenty_four_hours": counts.events_in_last_twenty_four_hours,
                    "sessions_in_last_twenty_four_hours": counts.sessions_in_last_twenty_four_hours,
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

#[derive(QueryableByName, Serialize)]
struct HourlyEventSummary {
    #[diesel(sql_type = Timestamp)]
    hour: NaiveDateTime,
    #[diesel(sql_type = Integer)]
    count: i32,
}

pub async fn hourly(pool: web::Data<DbPool>) -> impl Responder {
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
            eprintln!("Database query failed: {:?}", e);
            HttpResponse::InternalServerError().json(format!("Database error: {:?}", e))
        }
    }
}

#[derive(Serialize, Deserialize, QueryableByName)]
pub struct UrlEventCount {
    #[diesel(sql_type = Text)]
    pub url: String,
    #[diesel(sql_type = BigInt)]
    pub count: i64,
}

pub async fn urls(pool: web::Data<DbPool>) -> impl Responder {
    let start_time = Utc::now().naive_utc() - Duration::days(7);
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let sql = "
        SELECT url, COUNT(*) AS count
        FROM events
        WHERE timestamp > ?
        GROUP BY url
        ORDER BY count DESC
        LIMIT 25;
    ";

    let results: Result<Vec<UrlEventCount>, diesel::result::Error> = diesel::sql_query(sql)
        .bind::<Timestamp, _>(start_time)
        .load(&mut conn);

    match results {
        Ok(url_counts) => HttpResponse::Ok().json(url_counts),
        Err(e) => {
            eprintln!("Database query failed: {:?}", e);
            HttpResponse::InternalServerError().json(format!("Database error: {:?}", e))
        }
    }
}

#[derive(Serialize, Deserialize, QueryableByName)]
pub struct BrowserVisitCount {
    #[diesel(sql_type = Text)]
    pub browser: String,
    #[diesel(sql_type = BigInt)]
    pub count: i64,
}

pub async fn browsers(pool: web::Data<DbPool>) -> impl Responder {
    let start_time = Utc::now().naive_utc() - Duration::days(7);
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let sql = "
        SELECT browser, COUNT(*) AS count
        FROM collectors
        WHERE timestamp > ?
        AND browser IS NOT NULL
        GROUP BY browser
        ORDER BY count DESC
        LIMIT 25;
    ";

    let results: Result<Vec<BrowserVisitCount>, diesel::result::Error> = diesel::sql_query(sql)
        .bind::<Timestamp, _>(start_time)
        .load(&mut conn);

    match results {
        Ok(browser_counts) => HttpResponse::Ok().json(browser_counts),
        Err(e) => {
            eprintln!("Database query failed: {:?}", e);
            HttpResponse::InternalServerError().json(format!("Database error: {:?}", e))
        }
    }
}

#[derive(Serialize, Deserialize, QueryableByName)]
pub struct OsBrowserVisitCount {
    #[diesel(sql_type = Text)]
    os: String,
    #[diesel(sql_type = Text)]
    browser: String,
    #[diesel(sql_type = BigInt)]
    count: i64,
}

pub async fn os_browsers(pool: web::Data<DbPool>) -> impl Responder {
    let start_time = Utc::now().naive_utc() - Duration::days(7);
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let sql = "
    SELECT os, browser, COUNT(*) AS count
    FROM collectors
    WHERE timestamp > ?
    AND os IS NOT NULL
    AND browser IS NOT NULL
    GROUP BY os, browser
    ORDER BY count DESC
    LIMIT 25;
";

    let results: Result<Vec<OsBrowserVisitCount>, diesel::result::Error> = diesel::sql_query(sql)
        .bind::<Timestamp, _>(start_time)
        .load(&mut conn);

    match results {
        Ok(os_browser_counts) => HttpResponse::Ok().json(os_browser_counts),
        Err(e) => {
            eprintln!("Database query failed: {:?}", e);
            HttpResponse::InternalServerError().json(format!("Database error: {:?}", e))
        }
    }
}

#[derive(Serialize, Deserialize, QueryableByName)]
pub struct ReferrerCount {
    #[diesel(sql_type = Text)]
    domain: String,
    #[diesel(sql_type = BigInt)]
    count: i64,
}

pub async fn referrers(pool: web::Data<DbPool>) -> impl Responder {
    let start_time = Utc::now().naive_utc() - Duration::days(7);
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let sql = "
    SELECT 
    CASE 
        WHEN referrer IS NULL OR referrer = '' THEN 'direct'
        ELSE COALESCE(NULLIF(SUBSTR(referrer, INSTR(referrer, '//') + 2), ''), referrer) 
    END AS domain,
    COUNT(*) AS count
    FROM events
    WHERE timestamp > ?
    GROUP BY domain
    ORDER BY count DESC
    LIMIT 25;
    ";

    let results: Result<Vec<ReferrerCount>, diesel::result::Error> = diesel::sql_query(sql)
        .bind::<Timestamp, _>(start_time)
        .load(&mut conn);

    match results {
        Ok(referrer_counts) => HttpResponse::Ok().json(referrer_counts),
        Err(e) => {
            eprintln!("Database query failed: {:?}", e);
            HttpResponse::InternalServerError().json(format!("Database error: {:?}", e))
        }
    }
}

#[derive(QueryableByName, Serialize, Deserialize)]
pub struct HourlyEventCounts {
    #[diesel(sql_type = Integer)]
    pub day: i32, // 0 is Sunday and 6 is Saturday
    #[diesel(sql_type = Integer)]
    pub hour: i32, // Hour of the day (0-23)
    #[diesel(sql_type = BigInt)]
    pub count: i64, // The count of events in that hour
}

pub async fn weekly(pool: web::Data<DbPool>) -> impl Responder {
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    let query = diesel::sql_query(
        "SELECT \
        CAST(strftime('%w', timestamp) AS INTEGER) AS day, \
        CAST(strftime('%H', timestamp) AS INTEGER) AS hour, \
        COUNT(*) as count \
        FROM events \
        WHERE timestamp >= datetime('now', '-7 days') \
        GROUP BY day, hour",
    );

    match query.load::<HourlyEventCounts>(&mut conn) {
        Ok(hourly_counts) => HttpResponse::Ok().json(hourly_counts),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": format!("Error querying hourly event counts: {:?}", e)
        })),
    }
}

#[derive(QueryableByName, Debug)]
struct TrafficChange {
    #[diesel(sql_type = BigInt)]
    current_count: i64,
    #[diesel(sql_type = BigInt)]
    previous_count: i64,
}

pub async fn percentages(pool: web::Data<DbPool>) -> impl Responder {
    let mut conn = pool.get().expect("couldn't get db connection from pool");

    // calculates percentage change
    let calc_percentage_change = |current: i64, previous: i64| -> f64 {
        if previous == 0 {
            if current == 0 {
                0.0
            } else {
                f64::INFINITY
            }
        } else {
            ((current as f64 - previous as f64) / previous as f64) * 100.0
        }
    };

    let results = vec![
        ("day", "-1 day", "-2 days"),
        ("week", "-7 days", "-14 days"),
        ("month", "-1 month", "-2 months"),
    ]
    .iter()
    .map(|(label, current_interval, previous_interval)| {
        let query = diesel::sql_query(format!(
            "SELECT \
                (SELECT COUNT(*) FROM events WHERE timestamp >= datetime('now', '{}')) AS current_count, \
                (SELECT COUNT(*) FROM events WHERE timestamp BETWEEN datetime('now', '{}') AND datetime('now', '{}')) AS previous_count",
            current_interval, previous_interval, current_interval
        ));
        let result = query.get_results::<TrafficChange>(&mut conn);
        match result {
            Ok(tc) if !tc.is_empty() => {
                // Ensure there is at least one result to prevent division by zero
                let total_change = tc.iter().map(|traffic_change| calc_percentage_change(traffic_change.current_count, traffic_change.previous_count)).sum::<f64>() / tc.len() as f64;
                Ok((*label, total_change))
            },
            Ok(_) => Ok((*label, 0.0)), // No change if no data
            Err(e) => Err(e),
        }
    })
    .collect::<Result<Vec<(&str, f64)>, diesel::result::Error>>();

    match results {
        Ok(changes) => {
            let response: serde_json::Value = changes
                .into_iter()
                .map(|(label, change)| (label.to_string(), json!(change)))
                .collect::<serde_json::Map<String, serde_json::Value>>()
                .into();
            HttpResponse::Ok().json(response)
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": format!("Error querying event traffic changes: {:?}", e)
        })),
    }
}
