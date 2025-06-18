use crate::db::DbPool;
use actix_web::{web, HttpResponse, Responder, HttpRequest};
use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Text, Timestamp};
use log::{info, warn, error, debug};
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

pub async fn five_minutes(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 Five-minute summary request - IP: {}", client_ip);

    let start_time = Utc::now().naive_utc() - Duration::days(1);
    debug!("📅 Querying events from last 24 hours (since: {})", start_time);

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for five-minute summary");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for five-minute summary - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Could not get DB connection");
        }
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
        Ok(summary) => {
            let total_events: i64 = summary.iter().map(|s| s.count).sum();
            info!(
                "✅ Five-minute summary generated - IP: {}, Intervals: {}, Total events: {}",
                client_ip, summary.len(), total_events
            );
            HttpResponse::Ok().json(summary)
        }
        Err(e) => {
            error!("❌ Five-minute summary query failed - IP: {}, Error: {}", client_ip, e);
            HttpResponse::InternalServerError().json(format!("Database error: {:?}", e))
        }
    }
}

pub async fn events(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 Event counts summary request - IP: {}", client_ip);

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for event counts");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for event counts - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

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
                info!(
                    "✅ Event counts retrieved - IP: {}, Sessions (24h): {}, Events (24h): {}, Events (1h): {}, Events (5m): {}",
                    client_ip, counts.sessions_in_last_twenty_four_hours, counts.events_in_last_twenty_four_hours,
                    counts.events_in_last_hour, counts.events_in_last_five_minutes
                );
                HttpResponse::Ok().json(json!({
                    "events_in_last_hour": counts.events_in_last_hour,
                    "events_in_last_five_minutes": counts.events_in_last_five_minutes,
                    "events_in_last_twenty_four_hours": counts.events_in_last_twenty_four_hours,
                    "sessions_in_last_twenty_four_hours": counts.sessions_in_last_twenty_four_hours,
                }))
            } else {
                warn!("⚠️ No event counts data available - IP: {}", client_ip);
                HttpResponse::Ok().json(json!({
                    "error": "No data available"
                }))
            }
        }
        Err(e) => {
            error!("❌ Event counts query failed - IP: {}, Error: {}", client_ip, e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Error querying events: {:?}", e)
            }))
        }
    }
}

#[derive(QueryableByName, Serialize)]
struct HourlyEventSummary {
    #[diesel(sql_type = Timestamp)]
    hour: NaiveDateTime,
    #[diesel(sql_type = Integer)]
    count: i32,
}

pub async fn hourly(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 Hourly summary request - IP: {}", client_ip);

    let start_time = Utc::now().naive_utc() - Duration::days(1);
    debug!("📅 Querying hourly events from last 24 hours (since: {})", start_time);

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for hourly summary");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for hourly summary - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Could not get DB connection");
        }
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
        Ok(summary) => {
            let total_events: i32 = summary.iter().map(|s| s.count).sum();
            info!(
                "✅ Hourly summary generated - IP: {}, Hours: {}, Total events: {}",
                client_ip, summary.len(), total_events
            );
            HttpResponse::Ok().json(summary)
        }
        Err(e) => {
            error!("❌ Hourly summary query failed - IP: {}, Error: {}", client_ip, e);
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

pub async fn urls(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 URL statistics request - IP: {}", client_ip);

    let start_time = Utc::now().naive_utc() - Duration::days(7);
    debug!("📅 Querying URL events from last 7 days (since: {})", start_time);

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for URL statistics");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for URL statistics - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

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
        Ok(url_counts) => {
            let total_events: i64 = url_counts.iter().map(|u| u.count).sum();
            info!(
                "✅ URL statistics generated - IP: {}, URLs: {}, Total events: {}",
                client_ip, url_counts.len(), total_events
            );
            HttpResponse::Ok().json(url_counts)
        }
        Err(e) => {
            error!("❌ URL statistics query failed - IP: {}, Error: {}", client_ip, e);
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

pub async fn browsers(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 Browser statistics request - IP: {}", client_ip);

    let start_time = Utc::now().naive_utc() - Duration::days(7);
    debug!("📅 Querying browser data from last 7 days (since: {})", start_time);

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for browser statistics");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for browser statistics - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

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
        Ok(browser_counts) => {
            let total_visits: i64 = browser_counts.iter().map(|b| b.count).sum();
            info!(
                "✅ Browser statistics generated - IP: {}, Browsers: {}, Total visits: {}",
                client_ip, browser_counts.len(), total_visits
            );
            HttpResponse::Ok().json(browser_counts)
        }
        Err(e) => {
            error!("❌ Browser statistics query failed - IP: {}, Error: {}", client_ip, e);
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

pub async fn os_browsers(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 OS/Browser statistics request - IP: {}", client_ip);

    let start_time = Utc::now().naive_utc() - Duration::days(7);
    debug!("📅 Querying OS/Browser data from last 7 days (since: {})", start_time);

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for OS/Browser statistics");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for OS/Browser statistics - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

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
        Ok(os_browser_counts) => {
            let total_visits: i64 = os_browser_counts.iter().map(|o| o.count).sum();
            info!(
                "✅ OS/Browser statistics generated - IP: {}, Combinations: {}, Total visits: {}",
                client_ip, os_browser_counts.len(), total_visits
            );
            HttpResponse::Ok().json(os_browser_counts)
        }
        Err(e) => {
            error!("❌ OS/Browser statistics query failed - IP: {}, Error: {}", client_ip, e);
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

pub async fn referrers(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 Referrer statistics request - IP: {}", client_ip);

    let start_time = Utc::now().naive_utc() - Duration::days(7);
    debug!("📅 Querying referrer data from last 7 days (since: {})", start_time);

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for referrer statistics");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for referrer statistics - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

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
        Ok(referrer_counts) => {
            let total_events: i64 = referrer_counts.iter().map(|r| r.count).sum();
            info!(
                "✅ Referrer statistics generated - IP: {}, Referrers: {}, Total events: {}",
                client_ip, referrer_counts.len(), total_events
            );
            HttpResponse::Ok().json(referrer_counts)
        }
        Err(e) => {
            error!("❌ Referrer statistics query failed - IP: {}, Error: {}", client_ip, e);
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

pub async fn weekly(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 Weekly heatmap data request - IP: {}", client_ip);
    debug!("📅 Querying hourly event counts from last 7 days");

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for weekly heatmap");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for weekly heatmap - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

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
        Ok(hourly_counts) => {
            let total_events: i64 = hourly_counts.iter().map(|h| h.count).sum();
            info!(
                "✅ Weekly heatmap data generated - IP: {}, Data points: {}, Total events: {}",
                client_ip, hourly_counts.len(), total_events
            );
            HttpResponse::Ok().json(hourly_counts)
        }
        Err(e) => {
            error!("❌ Weekly heatmap query failed - IP: {}, Error: {}", client_ip, e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Error querying hourly event counts: {:?}", e)
            }))
        }
    }
}

#[derive(QueryableByName, Debug)]
struct TrafficChange {
    #[diesel(sql_type = BigInt)]
    current_count: i64,
    #[diesel(sql_type = BigInt)]
    previous_count: i64,
}

pub async fn percentages(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 Traffic percentage changes request - IP: {}", client_ip);

    let mut conn = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for percentage calculations");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for percentage calculations - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

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

    debug!("📈 Calculating traffic changes for day, week, and month intervals");

    let results = vec![
        ("day", "-1 day", "-2 days"),
        ("week", "-7 days", "-14 days"),
        ("month", "-1 month", "-2 months"),
    ]
    .iter()
    .map(|(label, current_interval, previous_interval)| {
        debug!("📊 Querying {} traffic changes", label);
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
                let total_change = tc.iter().map(|traffic_change| {
                    let change = calc_percentage_change(traffic_change.current_count, traffic_change.previous_count);
                    debug!("📈 {} change - Current: {}, Previous: {}, Change: {:.2}%", 
                           label, traffic_change.current_count, traffic_change.previous_count, change);
                    change
                }).sum::<f64>() / tc.len() as f64;
                Ok((*label, total_change))
            },
            Ok(_) => {
                debug!("📊 No {} traffic data available", label);
                Ok((*label, 0.0)) // No change if no data
            },
            Err(e) => {
                error!("❌ Error querying {} traffic changes - IP: {}, Error: {}", label, client_ip, e);
                Err(e)
            },
        }
    })
    .collect::<Result<Vec<(&str, f64)>, diesel::result::Error>>();

    match results {
        Ok(changes) => {
            info!(
                "✅ Traffic percentage changes calculated - IP: {}, Changes: {:?}",
                client_ip, changes
            );
            let response: serde_json::Value = changes
                .into_iter()
                .map(|(label, change)| (label.to_string(), json!(change)))
                .collect::<serde_json::Map<String, serde_json::Value>>()
                .into();
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            error!("❌ Traffic percentage changes query failed - IP: {}, Error: {}", client_ip, e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Error querying event traffic changes: {:?}", e)
            }))
        }
    }
}
