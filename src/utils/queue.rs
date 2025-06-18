use crate::db::DbPool;
use crate::models::NewEvent;
use diesel::prelude::*;
use log::{info, warn, error, debug};
use tokio::sync::mpsc::Receiver;
use tokio::task;
use tokio::time::{interval, Duration};

pub async fn process_events_async(mut rx: Receiver<NewEvent>, db_pool: DbPool) {
    let batch_size = 100;
    let batch_timeout = Duration::from_secs(5);

    info!("🚀 Event processing queue started - Batch size: {}, Timeout: {}s", batch_size, batch_timeout.as_secs());

    let mut interval = interval(batch_timeout);
    let mut batch: Vec<NewEvent> = Vec::new();

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                debug!("📥 Event received in queue - ID: {}, URL: {}, Event: {}", event.id, event.url, event.name);
                batch.push(event);
                debug!("📦 Current batch size: {}/{}", batch.len(), batch_size);
                
                if batch.len() >= batch_size {
                    info!("📦 Batch size limit reached ({}), processing batch", batch_size);
                    let db_pool_clone = db_pool.clone();
                    let batch_to_insert = std::mem::replace(&mut batch, Vec::new());
                    insert_batch(batch_to_insert, db_pool_clone).await;
                }
            },
            _ = interval.tick() => {
                if !batch.is_empty() {
                    let batch_len = batch.len();
                    info!("⏰ Batch timeout reached, processing {} events", batch_len);
                    let db_pool_clone = db_pool.clone();
                    let batch_to_insert = std::mem::replace(&mut batch, Vec::new());
                    insert_batch(batch_to_insert, db_pool_clone).await;
                } else {
                    debug!("⏰ Batch timeout reached but no events to process");
                }
            },
        }
    }
}

async fn insert_batch(batch: Vec<NewEvent>, db_pool: DbPool) {
    let batch_size = batch.len();
    let event_ids: Vec<String> = batch.iter().map(|e| e.id.clone()).collect();
    
    debug!("💾 Starting batch insert - Events: {}, IDs: {:?}", batch_size, event_ids);

    // Use `spawn_blocking` to move the blocking operation off the async executor
    let result = task::spawn_blocking(move || {
        let mut conn = match db_pool.get() {
            Ok(conn) => {
                debug!("🔗 Database connection established for batch insert");
                conn
            }
            Err(e) => {
                error!("❌ Failed to get database connection for batch insert: {}", e);
                return Err(format!("Database connection failed: {}", e));
            }
        };

        // Use `batch` directly as it is now owned by this closure
        match diesel::insert_into(crate::schema::events::table)
            .values(&batch)
            .execute(&mut conn)
        {
            Ok(rows_affected) => {
                debug!("💾 Batch insert executed - Rows affected: {}", rows_affected);
                Ok(rows_affected)
            }
            Err(e) => {
                error!("❌ Batch insert failed: {}", e);
                Err(format!("Insert failed: {}", e))
            }
        }
    })
    .await;

    match result {
        Ok(insert_result) => match insert_result {
            Ok(rows_affected) => {
                info!("✅ Batch inserted successfully - Events: {}, Rows affected: {}", batch_size, rows_affected);
            }
            Err(e) => {
                error!("❌ Batch insert failed - Events: {}, Error: {}", batch_size, e);
            }
        },
        Err(e) => {
            error!("❌ Failed to execute batch insert task - Events: {}, Error: {}", batch_size, e);
        }
    }
}
