#[macro_use] extern crate rocket;

use rocket::http::Status;
use rocket::response::stream::{EventStream, Event};
use std::sync::atomic::{AtomicUsize, Ordering};
use rocket::{State, Shutdown};
use rocket::tokio::sync::broadcast::{channel, Sender, error::RecvError};
use rocket::tokio::select;
use std::fs;

struct HitCount {
    count: AtomicUsize
}

#[get("/click")]
fn click(hit_count: &State<HitCount>, queue: &State<Sender<String>>) -> Status {
    let count = hit_count.count.fetch_add(1, Ordering::AcqRel) + 1;
    save_count(count);
    let _ = queue.send(count.to_string());
    Status::Ok
}

#[get("/count")]
fn count(hit_count: &State<HitCount>) -> String {
    hit_count.count.load(Ordering::Relaxed).to_string()
}

#[get("/events")]
async fn events(queue: &State<Sender<String>>, mut end: Shutdown) -> EventStream![] {
    let mut rx = queue.subscribe();
    EventStream! {
        loop {
            let msg = select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue
                },
                _ = &mut end => break
            };
            yield Event::data(msg);
        }
    }
}

fn saved_count() -> usize {
    let data = fs::read_to_string("./count.txt").unwrap_or("0".to_string());
    data.parse::<usize>().unwrap_or(0)
}

fn save_count(count: usize) {
    fs::write("./count.txt", count.to_string()).expect("Unable to write file...");
}

#[launch]
fn rocket() -> _ {
    let init_count = saved_count();
    rocket::build()
        .manage(HitCount { count: AtomicUsize::new(init_count) })
        .manage(channel::<String>(1024).0)
        .mount("/", routes![click, count, events])
}