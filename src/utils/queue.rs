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

impl NewEvent {
    pub fn estimated_total_size(&self) -> usize {
        let base_size = std::mem::size_of::<Self>();
        let strings_heap_size = self.id.capacity() * std::mem::size_of::<u8>()
            + self.url.capacity() * std::mem::size_of::<u8>()
            + self.name.capacity() * std::mem::size_of::<u8>()
            + self.collector_id.capacity() * std::mem::size_of::<u8>();
        base_size + strings_heap_size
    }
}

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
        let item_size = item.estimated_total_size();

        // Check if adding this event would exceed the memory limit
        if self.current_memory_bytes + item_size > self.max_memory_bytes {
            warn!("Event buffer limit exceeded, event ignored: {}", item.id);
            return Err("Memory limit exceeded");
        }

        // If the memory limit check passes, add the event to the queue
        self.queue.push_back(item);
        self.current_memory_bytes += item_size;

        Ok(())
    }
}

pub fn start_processing_events(pool: web::Data<DbPool>, queue: Arc<Mutex<EventsQueue>>) {
    thread::spawn(move || loop {
        // Attempt to lock the queue, handling potential lock poisoning.
        let mut queue_guard = match queue.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("Queue mutex was poisoned. Error: {:?}", poisoned);
                // Here you could choose to continue and attempt to recover the lock,
                // or implement a recovery strategy as needed.
                poisoned.into_inner()
            }
        };

        // Prepare a batch of events to insert.
        let mut events_to_insert = Vec::new();
        while let Some(event) = queue_guard.queue.pop_front() {
            let event_size = event.estimated_total_size(); // Calculate size before the move
            events_to_insert.push(event); // `event` is moved here
            queue_guard.current_memory_bytes -= event_size; // Use the previously calculated size
        }

        drop(queue_guard); // Explicitly drop to release the lock before potentially blocking on DB operations.

        // Process the batch of events.
        if !events_to_insert.is_empty() {
            let conn_result = pool.get();
            match conn_result {
                Ok(mut conn) => {
                    match diesel::insert_into(events)
                        .values(&events_to_insert)
                        .execute(&mut *conn)
                    {
                        Ok(count) => info!("Successfully processed {} events.", count),
                        Err(e) => error!("Failed to insert events into database. Error: {:?}", e),
                    }
                }
                Err(e) => error!("Failed to get DB connection from pool. Error: {:?}", e),
            }
        }

        // Sleep before the next iteration to avoid constant locking and unlocking in empty queue scenarios.
        thread::sleep(Duration::from_secs(5));
    });
}
