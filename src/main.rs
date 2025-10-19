use anyhow::Result;
use axum::Router;
use diesel_migrations::{EmbeddedMigrations, embed_migrations};
use medbook_core::{
    bootstrap::{self, bootstrap},
    config, db,
};
use medbook_orderservice::{consumers, routes};

/// Migrations embedded into the binary which helps with streamlining image building process
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[tokio::main]
async fn main() -> Result<()> {
    bootstrap::init_tracing();
    bootstrap::init_env();

    let app = Router::new()
        .merge(routes::patients::orders::routes())
        .merge(routes::patients::carts::routes())
        .merge(routes::payments::routes());

    tracing::info!("Running migrations...");
    let config = config::load()?;
    let migrations_count = db::run_migrations_blocking(MIGRATIONS, &config.database.url).await?;
    tracing::info!("Run {} new migrations successfully", migrations_count);

    tracing::info!("Bootstrapping...");
    bootstrap(
        "OrderService",
        app,
        &[
            ("orders.order_rejected", consumers::orders::order_rejected),
            ("orders.order_reserved", consumers::orders::order_reserved),
            (
                "orders.delivery_created",
                consumers::orders::delivery_created,
            ),
            (
                "orders.delivery_success",
                consumers::orders::delivery_success,
            ),
            (
                "orders.order_cancelled",
                consumers::orders::order_cancel_success,
            ),
        ],
    )
    .await?;
    Ok(())
}
