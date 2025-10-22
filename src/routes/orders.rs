use std::collections::HashMap;

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
        OpenApiRouter::new()
            .routes(utoipa_axum::routes!(get_order))
            .routes(utoipa_axum::routes!(get_orders)),
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

/// Fetch all orders belonging to the authenticated patient.
#[utoipa::path(
    get,
    path = "/",
    tags = ["Orders"],
    responses(
        (status = 200, description = "List my orders", body = StdResponse<Vec<GetOrderRes>, String>)
    )
)]
async fn get_orders(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let orders: Vec<OrderEntity> = orders::table
        .order_by(orders::updated_at.desc())
        .get_results(conn)
        .await
        .context("Failed to get my orders")?;

    let cart_ids: Vec<i32> = orders.iter().map(|order| order.cart_id).collect();
    let order_items: Vec<CartItemEntity> = cart_items::table
        .filter(cart_items::cart_id.eq_any(&cart_ids))
        .get_results(conn)
        .await
        .context("Failed to get cart items")?;

    let cart_item_ids = order_items.iter().map(|item| item.product_id).collect();
    let unit_prices = get_product_unit_prices(state.http_client, cart_item_ids).await?;

    let mut group: HashMap<i32, Vec<CartItemEntity>> = HashMap::new();
    for item in order_items {
        group.entry(item.cart_id).or_default().push(item);
    }

    let order_with_items: Vec<GetOrderRes> = orders
        .into_iter()
        .map(|order| {
            let order_items = group.remove(&order.cart_id).unwrap_or_default();
            let total_price: f32 = order_items
                .iter()
                .map(|item| unit_prices.get(&item.product_id).copied().unwrap_or(0.0))
                .sum();
            GetOrderRes {
                order_items,
                order,
                total_price,
            }
        })
        .collect();

    Ok(StdResponse {
        data: Some(order_with_items),
        message: Some("Get my orders successfully"),
    })
}
