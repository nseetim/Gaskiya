mod anchor;
mod errors;
mod handlers;
mod models;
mod state;
mod trust;

use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer};
use engine::cuckoo::CuckooShardTable;
use sqlx::postgres::PgPoolOptions;
use std::sync::Mutex;
use std::time::Duration;

use anchor::IpfsClient;
use state::AppState;

/// Simulated logical shards (spec Section 3): five in-process "nodes."
const NUM_SHARDS: usize = 5;
const SHARD_BUCKET_CAPACITY: usize = 10_000;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let ipfs_api_base = std::env::var("IPFS_API_BASE").unwrap_or_else(|_| "http://127.0.0.1:5001".to_string());
    let ipfs_gateway_base =
        std::env::var("IPFS_GATEWAY_BASE").unwrap_or_else(|_| "http://127.0.0.1:8081".to_string());
    // Short by production standards, but anchoring frequently narrows the
    // un-anchored-tamper window (spec Section 12) and keeps the demo snappy.
    let anchor_interval_secs: u64 = std::env::var("ANCHOR_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("../migrations").run(&pool).await?;

    // The Cuckoo table is in-process state (spec Section 3: simulated
    // logical shards), so it starts empty on every restart while
    // listing_versions.shard_id persists in the DB. Replaying inserts in
    // original creation order reconstructs an identical distribution,
    // since placement is a deterministic function of the key sequence.
    let mut shard_table = CuckooShardTable::new(NUM_SHARDS, SHARD_BUCKET_CAPACITY);
    let existing_listing_ids: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM opportunity_listings ORDER BY created_at ASC",
    )
    .fetch_all(&pool)
    .await?;
    for id in &existing_listing_ids {
        shard_table.insert(&id.to_string());
    }
    log::info!("replayed {} listing(s) into the shard table on startup", existing_listing_ids.len());

    let state = web::Data::new(AppState {
        pool,
        shard_table: Mutex::new(shard_table),
        ipfs: IpfsClient::new(ipfs_api_base),
        ipfs_gateway_base,
    });

    {
        let state = state.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(anchor_interval_secs));
            loop {
                ticker.tick().await;
                match anchor::run_anchor_batch(&state.pool, &state.ipfs).await {
                    Ok(Some(a)) => log::info!(
                        "anchored batch of {} version(s): root={} cid={:?}",
                        a.version_ids_included.len(),
                        a.batch_root_hash,
                        a.ipfs_cid
                    ),
                    Ok(None) => {}
                    Err(e) => log::error!("periodic anchor batch failed: {e}"),
                }
            }
        });
    }

    log::info!("gaskiya server listening on {bind_addr}");

    HttpServer::new(move || {
        // Permissive by hackathon-demo design: no cookies/credentials are
        // used (auth is a bearer-style UUID in the request body, not a
        // cookie), so a wide-open CORS policy doesn't expose session state.
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .app_data(state.clone())
            .route(
                "/health",
                web::get().to(|| async { HttpResponse::Ok().json(serde_json::json!({"status": "ok"})) }),
            )
            .route("/api/listings", web::post().to(handlers::create_listing))
            .route("/api/listings/{id}", web::get().to(handlers::get_listing))
            .route(
                "/api/listings/{id}/versions",
                web::post().to(handlers::create_version),
            )
            .route(
                "/api/listings/{id}/history",
                web::get().to(handlers::get_history),
            )
            .route(
                "/api/listings/{id}/verify",
                web::get().to(handlers::verify_listing),
            )
            .route(
                "/api/listings/{id}/corroborate",
                web::post().to(handlers::corroborate_listing),
            )
            .route(
                "/api/listings/{id}/report",
                web::post().to(handlers::report_listing),
            )
            .route("/api/search", web::get().to(handlers::search_listings))
            .route("/api/institutions", web::post().to(handlers::create_institution))
            .route("/api/institutions", web::get().to(handlers::list_institutions))
            .route(
                "/api/admin/listings/{id}/confirm-scam",
                web::post().to(handlers::confirm_scam),
            )
            .route("/api/watchlist", web::post().to(handlers::create_watch))
            .route(
                "/api/watchlist/{user_id}",
                web::get().to(handlers::get_watchlist),
            )
            .route(
                "/api/watchlist/entry/{watch_id}/acknowledge",
                web::post().to(handlers::acknowledge_watch),
            )
            .route(
                "/api/watchlist/entry/{watch_id}",
                web::delete().to(handlers::delete_watch),
            )
            .route(
                "/api/admin/shards",
                web::get().to(handlers::get_shard_distribution),
            )
            .route("/api/admin/anchors", web::get().to(handlers::list_anchors))
            .route(
                "/api/admin/anchors/run",
                web::post().to(handlers::trigger_anchor),
            )
    })
    .bind(&bind_addr)?
    .run()
    .await?;

    Ok(())
}
