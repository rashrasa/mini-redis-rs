use anyhow::Context;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::enable_raw_mode,
};
use log::{info, warn};
use mini_redis::serve;

const ENV_NAME_PROFILE: &str = "MINI_REDIS_PROFILE"; // true -> profile mode, else -> not profile mode

// TODO: Accept command-line arguments for testing (timer instead of ctrl-c to close), server config, etc.
// TODO: memory -> file sync policy
// TODO: Switch to HTTP
// TODO: Improve profiling tool to be more visual

#[tokio::main(flavor = "multi_thread", worker_threads = 16)]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Stdout)
        .init();

    let profile = match std::env::var(ENV_NAME_PROFILE) {
        Ok(v) => v.to_ascii_lowercase().trim() == "true",
        Err(_) => false,
    };

    clearscreen::clear().unwrap();

    if profile {
        warn!("Initializing tokio-console");
        console_subscriber::init();
    }

    let cancellation_token = tokio_util::sync::CancellationToken::new();

    let addr = std::env::var("MINI_REDIS_HOST").unwrap_or("192.168.2.30:3000".into());

    let cancellation_token_serve = cancellation_token.clone();
    let handle = tokio::spawn(async move {
        serve(&addr, cancellation_token_serve)
            .await
            .context("failed to start server")
            .unwrap();
    });

    // Wait to terminate
    // tokio::time::sleep(Duration::new(30, 0)).await;
    // tokio::signal::ctrl_c().await.unwrap();
    enable_raw_mode().unwrap();

    // TODO: try to find non-blocking solution
    println!("Press q to close");
    loop {
        if let Event::Key(key) = event::read().context("could not read keyboard input")?
            && key.code == KeyCode::Char('q')
        {
            break;
        }
    }

    info!("starting graceful shutdown");

    cancellation_token.cancel();

    handle.await.context("task failed")?;

    Ok(())
}
