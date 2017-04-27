// Database libraries.
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
extern crate r2d2;
extern crate r2d2_diesel;

// Serialization libraries.
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate serde;
extern crate bincode;
extern crate base64;

// Logging libraries.
#[macro_use] extern crate log;
extern crate badlog;
extern crate logger;

// Web framework.
extern crate iron; // Core.
extern crate persistent; // Shared memory middleware.
extern crate router; // Routing.

// Other libraries.
#[macro_use] extern crate juniper; // Query.
extern crate byteorder; // Numbers <-> bytes.
extern crate chrono; // Time.
extern crate lettre; // Email.
extern crate regex; // Regular expressions.
extern crate sodiumoxide; // Cryptography.
extern crate unicase; // Case-insensitivity.

use config::Config;
use database::Database;
use iron::headers::*;
use iron::prelude::*;
use iron::method::Method;
use iron::status;
use logger::Logger;
use persistent::Read;
use unicase::UniCase;
use std::net::Ipv4Addr;

mod auth;
mod conditions;
mod config;
mod database;
mod email;
mod routes;
mod schema;

//LONG: i18n
//LONG: Some kind of init script.
//TODO: birthday + weight + height
//TODO: gradings + belt
//LONG: public profiles
//LONG: SWITCH TO UUIDS BECAUSE SERIALS ARE A HUGE SECURITY ISSUE HERE.
//TODO: registration
//TODO: home address, phone, mobile
//TODO: emergency contacts

fn main() {
    // Initialize the environment.
    badlog::init_from_env("LOG_LEVEL");
    sodiumoxide::init();
    let config = Config::get();

    // Set up the database.
    let db = match Database::new(&config) {
        Ok(db) => db,
        Err(e) => {
            error!("Could not initialize the database: {}.", e);
            return;
        }
    };

    // Register some post-processing.
    let access = AccessControlAllowOrigin::Value(config.frontend_url.clone());
    let headers = AccessControlAllowHeaders(vec![
        UniCase("Authorization".to_owned()),
        UniCase("Content-Type".to_owned())
    ]);
    let methods = AccessControlAllowMethods(vec![Method::Get, Method::Post]);

    let process = move |req: &mut Request, mut res: Response| {
        if req.method == Method::Options {
            res = Response::with(status::Ok);
            res.headers.set(headers.clone());
            res.headers.set(methods.clone());
        }

        res.headers.set(access.clone());
        Ok(res)
    };

    // Start the server.
    let addr = (Ipv4Addr::new(0, 0, 0, 0), config.port);
    
    let mut chain = Chain::new(routes::build());
    chain.link_before(db);
    chain.link_before(Read::<Config>::one(config));
    chain.link_after(process);
    chain.link(Logger::new(None));

    match Iron::new(chain).http(addr) { //LONG: HTTPS/2
        Ok(x) => info!("Listening on {}!", x.socket),
        Err(e) => error!("Could not initialize server: {}.", e)
    }
}
