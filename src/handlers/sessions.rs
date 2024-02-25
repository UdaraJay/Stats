use crate::db::DbPool;
use crate::models::{Collector, Event};
use crate::schema::{collectors, events};
use actix_web::{web, HttpResponse, Responder};
use diesel::prelude::*;
use diesel::BelongingToDsl;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct CollectorWithEvents {
    collector: Collector,
    events: Vec<Event>,
}

pub async fn retrieve_sessions(pool: web::Data<DbPool>) -> impl Responder {
    let mut conn: diesel::r2d2::PooledConnection<
        diesel::r2d2::ConnectionManager<SqliteConnection>,
    > = pool.get().expect("couldn't get db connection from pool");

    let results = collectors::table
        .order(collectors::timestamp.desc())
        .limit(50)
        .load::<Collector>(&mut conn)
        .expect("Error loading collectors");

    let collector_ids: Vec<String> = results.iter().map(|c| c.id.clone()).collect();

    let events_for_collectors = Event::belonging_to(&results)
        .filter(events::collector_id.eq_any(collector_ids))
        .load::<Event>(&mut conn)
        .expect("Error loading events")
        .grouped_by(&results);

    let collectors_with_events: Vec<CollectorWithEvents> = results
        .into_iter()
        .zip(events_for_collectors)
        .map(|(collector, events)| CollectorWithEvents { collector, events })
        .collect();

    HttpResponse::Ok().json(collectors_with_events)
}
