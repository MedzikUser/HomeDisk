use std::{fs::File, path::Path};

use homedisk_database::Database;
use homedisk_server::serve_http;
use homedisk_types::config::Config;
use tracing::{info, warn};

mod logger;

pub const DATABASE_FILE: &str = "homedisk.db";

#[tokio::main]
async fn main() {
    // initialize logger
    logger::init();

    // parse config
    let config = Config::parse().expect("parse config");

    // open database connection
    let db =
        // if database file doesn't exists create it
        if !Path::new(DATABASE_FILE).exists() {
            warn!("Database file doesn't exists.");
            info!("Creating database file...");

            // create an empty database file
            File::create(DATABASE_FILE).expect("create a database file");

            // open database file
            let db = Database::open(DATABASE_FILE)
                .await
                .expect("open database file");

            // create tables in the database
            db.create_tables()
                .await
                .expect("create tables in the database");

            db
        }
        // if database file exists
        else {
            // open database connection
            Database::open(DATABASE_FILE)
                .await
                .expect("open database file")
        };

    // change the type from Vec<String> to Vec<HeaderValue> so that the http server can correctly detect CORS hosts
    let origins = config
        .http
        .cors
        .iter()
        .map(|e| e.parse().expect("parse CORS hosts"))
        .collect();

    // format host ip and port
    let host = format!(
        "{host}:{port}",
        host = config.http.host,
        port = config.http.port
    );

    // start http server
    serve_http(host, origins, db, config)
        .await
        .expect("start http server");
}
