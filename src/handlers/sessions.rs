use crate::db::DbPool;
use crate::models::{Collector, Event};
use crate::schema::{collectors, events};
use crate::utils::city::get_city_coordinates;
use actix_web::{web, HttpResponse, Responder};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel::sql_types::{BigInt, Text};
use diesel::BelongingToDsl;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct CollectorWithEvents {
    collector: Collector,
    events: Vec<Event>,
}

pub async fn retrieve_sessions(pool: web::Data<DbPool>) -> impl Responder {
    let mut conn: PooledConnection<ConnectionManager<SqliteConnection>> =
        pool.get().expect("couldn't get db connection from pool");

    let results = match collectors::table
        .order(collectors::timestamp.desc())
        .limit(30)
        .load::<Collector>(&mut conn)
    {
        Ok(results) => results,
        Err(e) => {
            eprintln!("Error loading collectors: {:?}", e);
            return HttpResponse::InternalServerError().json("Error loading collectors");
        }
    };

    // If there are no collectors, return an empty array
    if results.is_empty() {
        return HttpResponse::Ok().json(Vec::<CollectorWithEvents>::new());
    }

    let collector_ids: Vec<String> = results.iter().map(|c| c.id.clone()).collect();

    let events_for_collectors = match Event::belonging_to(&results)
        .filter(events::collector_id.eq_any(collector_ids))
        .load::<Event>(&mut conn)
    {
        Ok(events) => events,
        Err(e) => {
            eprintln!("Error loading events: {:?}", e);
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

pub async fn map(pool: web::Data<DbPool>) -> impl Responder {
    let mut conn: PooledConnection<ConnectionManager<SqliteConnection>> =
        pool.get().expect("couldn't get db connection from pool");

    let seven_days_ago = Utc::now().naive_utc() - Duration::days(7);

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
        Ok(results) => results,
        Err(e) => {
            eprintln!("Error querying city collector counts: {:?}", e);
            return HttpResponse::InternalServerError()
                .json("Error querying city collector counts");
        }
    };

    let max_count = results.iter().map(|c| c.count).max().unwrap_or(1);

    let mut city_counts: Vec<CityCollectorCount> = Vec::new();

    for city_count in results {
        if let Some((latitude, longitude)) = get_city_coordinates(&city_count.city) {
            let relative_size = city_count.count as f64 / max_count as f64;
            city_counts.push(CityCollectorCount {
                city: city_count.city,
                lat: latitude,
                lng: longitude,
                size: relative_size,
                color: "#fa4f33".to_string(),
            });
        } else {
            eprintln!("Coordinates not found for city: {}", city_count.city);
        }
    }

    HttpResponse::Ok().json(city_counts)
}
