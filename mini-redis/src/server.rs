use std::{path::Path, sync::Arc};

use anyhow::Context;
use axum::{Json, Router, extract::State, routing};
use log::info;
use tokio::{
    fs::{File, create_dir_all, try_exists},
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::sync::CancellationToken;

use crate::core::{Configuration, JsonFileHandler};

/// Creates a default config and serves clients.
pub async fn serve(host: &str, cancellation_token: CancellationToken) -> anyhow::Result<()> {
    let os_data_path = dirs::data_dir()
        .clone()
        .context("could not get data directory for current platform")?;

    let app_path = os_data_path.join("mini-redis-rs").join("default");

    create_dir_all(&app_path)
        .await
        .context(format!("could not create paths {:?}", &app_path))?;

    let config_path = app_path.join("config.json");

    let exists = try_exists(&config_path).await.context(format!(
        "could not check existence of path {:?}",
        &config_path
    ))?;

    if !exists {
        let mut file = File::create_new(&config_path)
            .await
            .context("could not create new data file")?;
        let bytes = serde_json::to_vec(&Configuration::default())
            .context("could not serialize default configuration")?;

        file.write_all(&bytes)
            .await
            .context("failed to write default configuration to default data file")?;
    }

    serve_from_file(config_path, host, cancellation_token).await
}

/// Serve connections using data from path.
///
/// # Cancellation Safety
///
/// This function is not cancel-safe. It must be cancelled through the
/// cancellation_token to ensure all data is written to disk.
pub async fn serve_from_file(
    config_path: impl AsRef<Path>,
    host: &str,
    cancellation_token: CancellationToken,
) -> anyhow::Result<()> {
    let config_path = config_path.as_ref();
    let mut config_bytes = vec![];
    File::open(&config_path)
        .await
        .context(format!("could not open config file at {:?}", config_path))?
        .read_to_end(&mut config_bytes)
        .await
        .context(format!("could not read config file at {:?}", config_path))?;

    let config = serde_json::from_slice::<Configuration>(&config_bytes)
        .context(format!("failed to parse config from {:?}", config_path))?;
    let pwd = config_path
        .parent()
        .context(format!("failed to read parent path at {:?}", config_path))?;
    let data_path = pwd.join(&config.data_path);

    let data = JsonFileHandler::from_path(data_path)
        .await
        .context(format!(
            "failed to read data file from {}",
            &config.data_path
        ))?;
    let listener = TcpListener::bind(host)
        .await
        .context(format!("failed to bind to {}", host))?;

    let state = Arc::new(crate::State { data });

    let router = Router::new()
        .route("/read", routing::get(read))
        .route("/insert", routing::post(insert))
        .route("/delete", routing::delete(delete))
        .with_state(Arc::clone(&state));

    info!("listening @ {}", host);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move { cancellation_token.cancelled().await })
        .await
        .context("server error")?;

    state.sync().await;

    Ok(())
}

#[axum::debug_handler]
async fn insert(
    State(s): State<Arc<crate::State>>,
    Json(payload): Json<crate::InsertRequest>,
) -> String {
    match s.data.write(&payload.key, payload.value).await {
        Some(old) => format!("Old value: {}", old),
        None => String::from("null"),
    }
}

#[axum::debug_handler]
async fn read(
    State(s): State<Arc<crate::State>>,
    Json(payload): Json<crate::InsertRequest>,
) -> String {
    match s.data.read(&payload.key).await {
        Some(v) => format!("{}", v),
        None => String::from("null"),
    }
}

#[axum::debug_handler]
async fn delete(
    State(s): State<Arc<crate::State>>,
    Json(payload): Json<crate::InsertRequest>,
) -> String {
    match s.data.delete(&payload.key).await {
        Some(v) => format!("{}", v),
        None => String::from("null"),
    }
}
