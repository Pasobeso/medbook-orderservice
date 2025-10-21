use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
};
use diesel::{ExpressionMethods, QueryDsl, QueryResult};
use diesel_async::RunQueryDsl;
use medbook_core::app_error::StdResponse;
use medbook_core::{aliases::DieselError, app_error::AppError, app_state::AppState};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;

use serde::Serialize;

use crate::{
    api::products::get_product_unit_prices,
    models::{CartItemEntity, OrderEntity},
    schema::{cart_items, orders},
};

pub fn routes_with_openapi() -> OpenApiRouter<AppState> {
    utoipa_axum::router::OpenApiRouter::new().nest(
        "/orders",
        OpenApiRouter::new().routes(utoipa_axum::routes!(get_order)),
    )
}

#[derive(Serialize, ToSchema)]
struct GetOrderRes {
    pub order: OrderEntity,
    pub order_items: Vec<CartItemEntity>,
    pub total_price: f32,
}

/// Fetch a specific order.
#[utoipa::path(
    get,
    path = "/{id}",
    tags = ["Orders"],
    params(
        ("id" = i32, Path, description = "Order ID to fetch")
    ),
    responses(
        (status = 200, description = "Get order successfully", body = StdResponse<GetOrderRes, String>)
    )
)]
async fn get_order(
    Path(id): Path<i32>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let order: QueryResult<OrderEntity> = orders::table.find(id).get_result(conn).await;

    if let Err(err) = order {
        match err {
            DieselError::NotFound => return Err(AppError::NotFound),
            _ => return Err(AppError::Other(err.into())),
        }
    }

    let order = order.unwrap();
    let order_items: Vec<CartItemEntity> = cart_items::table
        .filter(cart_items::cart_id.eq(order.cart_id))
        .get_results(conn)
        .await
        .context("Failed to get order items")?;

    let cart_item_ids = order_items.iter().map(|item| item.product_id).collect();
    let unit_prices = get_product_unit_prices(state.http_client, cart_item_ids).await?;
    let total_price: f32 = order_items
        .iter()
        .map(|item| unit_prices.get(&item.product_id).copied().unwrap_or(0.0))
        .sum();

    Ok(StdResponse {
        data: Some(GetOrderRes {
            order,
            order_items,
            total_price,
        }),
        message: Some("Get order successfully"),
    })
}
