use crate::db::DbPool;
use crate::models::{Collector, Event};
use crate::schema::{collectors, events};
use crate::utils::city::get_city_coordinates;
use actix_web::{web, HttpResponse, Responder, HttpRequest};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel::sql_types::{BigInt, Text};
use diesel::BelongingToDsl;
use log::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct CollectorWithEvents {
    collector: Collector,
    events: Vec<Event>,
}

pub async fn retrieve_sessions(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("📊 Sessions retrieval request - IP: {}", client_ip);

    let mut conn: PooledConnection<ConnectionManager<SqliteConnection>> = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for sessions retrieval");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for sessions retrieval - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

    let results = match collectors::table
        .order(collectors::timestamp.desc())
        .limit(30)
        .load::<Collector>(&mut conn)
    {
        Ok(results) => {
            debug!("✅ Retrieved {} collectors from database", results.len());
            results
        }
        Err(e) => {
            error!("❌ Error loading collectors - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::InternalServerError().json("Error loading collectors");
        }
    };

    // If there are no collectors, return an empty array
    if results.is_empty() {
        info!("📊 No collectors found - IP: {}", client_ip);
        return HttpResponse::Ok().json(Vec::<CollectorWithEvents>::new());
    }

    let collector_ids: Vec<String> = results.iter().map(|c| c.id.clone()).collect();
    debug!("🔍 Looking up events for {} collectors", collector_ids.len());

    let events_for_collectors = match Event::belonging_to(&results)
        .filter(events::collector_id.eq_any(collector_ids))
        .load::<Event>(&mut conn)
    {
        Ok(events) => {
            debug!("✅ Retrieved {} events for collectors", events.len());
            events
        }
        Err(e) => {
            error!("❌ Error loading events - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::InternalServerError().json("Error loading events");
        }
    }
    .grouped_by(&results);

    // Remove collectors with no events
    let collectors_with_events: Vec<CollectorWithEvents> = results
        .into_iter()
        .zip(events_for_collectors)
        .filter(|(_, events)| !events.is_empty())
        .map(|(collector, events)| CollectorWithEvents { collector, events })
        .collect();

    info!(
        "✅ Sessions retrieval successful - IP: {}, Sessions: {}, Total events: {}",
        client_ip,
        collectors_with_events.len(),
        collectors_with_events.iter().map(|c| c.events.len()).sum::<usize>()
    );

    HttpResponse::Ok().json(collectors_with_events)
}

#[derive(QueryableByName)]
pub struct CityCount {
    #[sql_type = "Text"]
    pub city: String,
    #[sql_type = "BigInt"]
    pub count: i64,
}

#[derive(Serialize, Deserialize)]
pub struct CityCollectorCount {
    pub lat: f64,
    pub lng: f64,
    pub size: f64,
    pub color: String,
    pub city: String,
}

pub async fn map(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    info!("🗺️ Map data request - IP: {}", client_ip);

    let mut conn: PooledConnection<ConnectionManager<SqliteConnection>> = match pool.get() {
        Ok(conn) => {
            debug!("🔗 Database connection established for map data");
            conn
        }
        Err(e) => {
            error!("❌ Failed to get database connection for map data - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::ServiceUnavailable().json("Database connection failed");
        }
    };

    let seven_days_ago = Utc::now().naive_utc() - Duration::days(7);
    debug!("📅 Querying collectors from last 7 days (since: {})", seven_days_ago);

    let query = r#"
        SELECT city, COUNT(*) as count
        FROM collectors
        WHERE timestamp > ?
        GROUP BY city
    "#;

    let results: Vec<CityCount> = match diesel::sql_query(query)
        .bind::<diesel::sql_types::Timestamp, _>(seven_days_ago)
        .load::<CityCount>(&mut conn)
    {
        Ok(results) => {
            debug!("✅ Retrieved city counts: {} unique cities", results.len());
            results
        }
        Err(e) => {
            error!("❌ Error querying city collector counts - IP: {}, Error: {}", client_ip, e);
            return HttpResponse::InternalServerError()
                .json("Error querying city collector counts");
        }
    };

    let max_count = results.iter().map(|c| c.count).max().unwrap_or(1);
    debug!("📊 Max city count: {}", max_count);

    let mut city_counts: Vec<CityCollectorCount> = Vec::new();
    let mut cities_with_coords = 0;
    let mut cities_without_coords = 0;

    for city_count in results {
        if let Some((latitude, longitude)) = get_city_coordinates(&city_count.city) {
            let relative_size = city_count.count as f64 / max_count as f64;
            city_counts.push(CityCollectorCount {
                city: city_count.city.clone(),
                lat: latitude,
                lng: longitude,
                size: relative_size,
                color: "#fa4f33".to_string(),
            });
            cities_with_coords += 1;
            debug!(
                "🏙️ City mapped - {}: {} visits, relative size: {:.2}",
                city_count.city, city_count.count, relative_size
            );
        } else {
            cities_without_coords += 1;
            warn!("⚠️ Coordinates not found for city: {}", city_count.city);
        }
    }

    info!(
        "✅ Map data generated - IP: {}, Cities with coords: {}, Cities without coords: {}",
        client_ip, cities_with_coords, cities_without_coords
    );

    HttpResponse::Ok().json(city_counts)
}
