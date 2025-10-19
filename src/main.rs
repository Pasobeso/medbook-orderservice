use anyhow::Result;
use axum::Router;
use medbook_core::bootstrap::bootstrap;
use medbook_orderservice::{consumers, routes};

#[tokio::main]
async fn main() -> Result<()> {
    let app = Router::new()
        .merge(routes::patients::orders::routes())
        .merge(routes::patients::carts::routes())
        .merge(routes::payments::routes());

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
