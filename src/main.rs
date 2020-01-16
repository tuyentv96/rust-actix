//! Application may have multiple data objects that are shared across
//! all handlers within same Application.
//!
//! For global shared state, we wrap our state in a `actix_web::web::Data` and move it into
//! the factory closure. The closure is called once-per-thread, and we clone our state
//! and attach to each instance of the `App` with `.app_data(state.clone())`.
//!
//! For thread-local state, we construct our state within the factory closure and attach to
//! the app with `.data(state)`.
//!
//! We retrieve our app state within our handlers with a `state: Data<...>` argument.
//!
//! By default, `actix-web` runs one `App` per logical cpu core.
//! When running on <N> cores, we see that the example will increment `counter1` (global state)
//! each time the endpoint is called, but only appear to increment `counter2` every
//! Nth time on average (thread-local state). This is because the workload is being shared
//! equally among cores.
//!
//! Check [user guide](https://actix.rs/docs/application/#state) for more info.

#[macro_use]
extern crate diesel;
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

mod model;
mod schema;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use dotenv;
use std::cell::Cell;
use std::io;
use std::sync::Mutex;
use uuid::Uuid;

use self::schema::stores::dsl::*;
use crate::model::{NewStore, Store};
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use serde_json::Value;

type Pool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// simple handle
async fn index(req: HttpRequest) -> HttpResponse {
    println!("{:?}", req);

    // Increment the counters

    let body = format!("global counter: local counter:");
    HttpResponse::Ok().body(body)
}

fn create_store(
    request_data: web::Json<serde_json::Value>,
    pool: web::Data<Pool>,
) -> HttpResponse {
    let serialized = request_data.to_string();
    let uuid = format!("{}", uuid::Uuid::new_v4());
    let new_entry = NewStore {
        data: &serialized,
        api_id: &uuid,
    };
    let conn = &pool.get().unwrap();
    if let Ok(_) = diesel::insert_into(stores).values(&new_entry).execute(conn) {
        if let Ok(mut result) = stores.load::<model::Store>(conn) {
            return HttpResponse::Ok().json::<Value>(result.pop().unwrap().into());
        }
    }
    HttpResponse::InternalServerError().into()
}

#[actix_rt::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    dotenv::dotenv().ok();

    let connspec = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let manager = ConnectionManager::<PgConnection>::new(connspec);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    // move is necessary to give closure below ownership of counter1
    HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            // .app_data(counter1.clone()) // add shared state
            // .data(counter2) // add thread-local state
            // enable logger
            .wrap(middleware::Logger::default())
            // register simple handler
            .service(web::resource("/").to(index))
            // .service(web::resource("/store").route(web::get().to(index)))
            .service(web::resource("/store").route(web::post().to(create_store)))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
