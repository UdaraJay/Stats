use crate::db::DbPool;
use crate::models::NewEvent;
use crate::schema::events::dsl::*;
use actix_web::web;
use diesel::prelude::*;
use log::{error, info, warn};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct EventsQueue {
    queue: VecDeque<NewEvent>,
    max_memory_bytes: usize,
    current_memory_bytes: usize,
}

// We have a X mb event queue that will act as a buffer between the collector and the database.
// Tweak as neccessary: 4MB buffers around 50K events and the queue is processed in batches every 5 seconds,
// so it should be enough for most small to medium traffic use-cases.
impl EventsQueue {
    pub fn new(max_memory_bytes: usize) -> Self {
        EventsQueue {
            queue: VecDeque::new(),
            max_memory_bytes,
            current_memory_bytes: 0,
        }
    }

    pub fn push(&mut self, item: NewEvent) -> Result<(), &'static str> {
        let item_size = std::mem::size_of::<NewEvent>(); // Estimate~

        info!("max_memory_bytes {}", self.max_memory_bytes);
        info!("current_memory_bytes {}", self.current_memory_bytes);
        info!("item_size {}", item_size);
        if self.current_memory_bytes + item_size > self.max_memory_bytes {
            // We ignore new events when cap is hit
            warn!("Memory limit exceeded, event ignored: {}", item.id);
            Ok(())
        } else {
            // Log the event ID before pushing the item into the queue
            info!("Event pushed to queue: {}", item.id);
            self.queue.push_back(item);
            self.current_memory_bytes += item_size;
            Ok(())
        }
    }
}

pub fn start_processing_events(pool: web::Data<DbPool>, queue: Arc<Mutex<EventsQueue>>) {
    thread::spawn(move || loop {
        let mut events_to_insert = Vec::new();
        {
            let mut queue = queue.lock().unwrap();
            while let Some(event) = queue.queue.pop_front() {
                events_to_insert.push(event);
                queue.current_memory_bytes -= std::mem::size_of::<NewEvent>();
            }
        }

        if !events_to_insert.is_empty() {
            let mut conn = pool.get().expect("Couldn't get db connection from pool");
            match diesel::insert_into(events)
                .values(&events_to_insert)
                .execute(&mut *conn)
            {
                Ok(_) => info!("Processed {} events from queue", events_to_insert.len()),
                Err(e) => error!("Error inserting events into database: {}", e),
            }
        }

        thread::sleep(Duration::from_secs(5));
    });
}
