use std::{collections::HashMap, fs::File, sync::Arc};

use actix_web::{
    App, Responder, get,
    middleware::Logger,
    post,
    web::{self, Data},
};
use clap::Parser;
use log::{debug, info};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
struct Args {
    /// Path to json file.
    #[arg(short, long)]
    path: String,
}

struct AppState {
    data: Arc<RwLock<HashMap<String, Value>>>,
}
#[derive(Deserialize)]
struct PostEntriesInsertData {
    value: Value,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Stdout)
        .init();

    debug!("Initialized logger");
    let path = match Args::try_parse() {
        Ok(a) => a.path,
        Err(_) => String::from("sample_data.json"),
    };

    if let Ok(_) = File::create_new(&path) {
        tokio::fs::write(&path, "{}").await.unwrap();
        debug!("Created file {}", &path);
    };
    debug!("Parsing json file @ {}", &path);
    let json_str = tokio::fs::read_to_string(&path).await.unwrap();
    let data: Arc<RwLock<HashMap<String, Value>>> =
        Arc::new(RwLock::new(serde_json::from_str(&json_str).unwrap()));

    info!("Starting HTTP server");
    actix_web::HttpServer::new(move || {
        App::new()
            .app_data(Data::new(AppState { data: data.clone() }))
            .service(get_entries)
            .service(post_entries)
            .wrap(Logger::default())
    })
    .bind(("127.0.0.1", 3000))
    .unwrap()
    .run()
    .await
    .unwrap();
    Ok(())
}

#[get("/entries/{key}")]
async fn get_entries(data: web::Data<AppState>, key: web::Path<String>) -> impl Responder {
    let json = data.data.read().await;

    if let Some(v) = json.get(key.as_str()) {
        format!("{}", v)
    } else {
        format!("Could not find value with key {}.", &key)
    }
}

#[post("/entries/insert/{key}")]
async fn post_entries(
    data: web::Data<AppState>,
    key: web::Path<String>,
    web::Form(info): web::Form<PostEntriesInsertData>,
) -> impl Responder {
    data.data
        .write()
        .await
        .insert(format!("{}", key), info.value.clone())
        .unwrap();
    return format!("Inserted {}: {} into the database.", key, info.value);
}
