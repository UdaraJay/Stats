use crate::db::DbPool;
use crate::models::NewEvent;
use diesel::prelude::*;
use tokio::sync::mpsc::Receiver;
use tokio::task;
use tokio::time::{interval, Duration};

pub async fn process_events_async(mut rx: Receiver<NewEvent>, db_pool: DbPool) {
    let batch_size = 100;
    let batch_timeout = Duration::from_secs(5);

    let mut interval = interval(batch_timeout);
    let mut batch: Vec<NewEvent> = Vec::new();

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                batch.push(event);
                if batch.len() >= batch_size {
                    let db_pool_clone = db_pool.clone();
                    let batch_to_insert = std::mem::replace(&mut batch, Vec::new());
                    insert_batch(batch_to_insert, db_pool_clone).await;
                }
            },
            _ = interval.tick() => {
                if !batch.is_empty() {
                    let db_pool_clone = db_pool.clone();
                    let batch_to_insert = std::mem::replace(&mut batch, Vec::new());
                    insert_batch(batch_to_insert, db_pool_clone).await;
                }
            },
        }
    }
}

async fn insert_batch(batch: Vec<NewEvent>, db_pool: DbPool) {
    // Use `spawn_blocking` to move the blocking operation off the async executor
    let result = task::spawn_blocking(move || {
        // Now that `batch` is owned, it can be moved into the closure safely
        let mut conn = db_pool
            .get()
            .expect("Failed to get DB connection from pool");

        // Use `batch` directly as it is now owned by this closure
        diesel::insert_into(crate::schema::events::table)
            .values(&batch)
            .execute(&mut conn)
    })
    .await
    .expect("Failed to execute block_in_place");

    match result {
        Ok(_) => println!("Batch inserted successfully."),
        Err(e) => eprintln!("Failed to insert batch: {:?}", e),
    }
}
