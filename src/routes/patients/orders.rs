use anyhow::{Context, Result};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing,
};
use diesel::{ExpressionMethods, QueryDsl, QueryResult, SelectableHelper};
use diesel_async::{AsyncConnection, RunQueryDsl};
use medbook_core::app_error::StdResponse;
use medbook_core::{
    aliases::DieselError,
    app_error::AppError,
    app_state::AppState,
    middleware::{self},
    outbox,
};
use medbook_events::OrderCancelledEvent;
use std::collections::HashMap;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;

use serde::{Deserialize, Serialize};

use crate::{
    api::{
        deliveries::get_delivery_address_as_value_with_ownership_check,
        products::get_product_unit_prices,
    },
    models::{CartItemEntity, CreateOrderEntity, CreatePaymentEntity, OrderEntity, PaymentEntity},
    schema::{
        cart_items::{self},
        orders::{self},
        payments::{self},
    },
};

/// Defines all patient-facing order routes (CRUD operations + authorization).
#[deprecated]
pub fn routes() -> Router<AppState> {
    Router::new().nest(
        "/patients/orders",
        Router::new()
            .route("/", routing::get(get_orders))
            .route("/", routing::post(create_order))
            .route("/my-orders", routing::get(get_my_orders))
            .route("/{id}", routing::get(get_order))
            .route("/{id}", routing::delete(cancel_order))
            .route("/{id}/payment", routing::post(create_payment_for_order))
            .route_layer(axum::middleware::from_fn(
                middleware::patients_authorization,
            )),
    )
}

/// Defines routes with OpenAPI specs. Should be used over `routes()` where possible.
pub fn routes_with_openapi() -> OpenApiRouter<AppState> {
    utoipa_axum::router::OpenApiRouter::new().nest(
        "/patients/orders",
        OpenApiRouter::new()
            .routes(utoipa_axum::routes!(get_orders))
            .routes(utoipa_axum::routes!(get_order))
            .routes(utoipa_axum::routes!(get_my_orders))
            .routes(utoipa_axum::routes!(create_order))
            .routes(utoipa_axum::routes!(cancel_order))
            .routes(utoipa_axum::routes!(create_payment_for_order))
            .route_layer(axum::middleware::from_fn(
                middleware::patients_authorization,
            )),
    )
}

/// Fetch all orders in the system.
#[utoipa::path(
    get,
    path = "/",
    tags = ["Orders"],
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "List all orders", body = StdResponse<Vec<OrderEntity>, String>)
    )
)]
async fn get_orders(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let orders: Vec<OrderEntity> = orders::table
        // .filter(orders::deleted_at.is_null())
        .get_results(conn)
        .await
        .context("Failed to get orders")?;

    Ok(StdResponse {
        data: Some(orders),
        message: Some("Get orders succesfully"),
    })
}

#[derive(Serialize, ToSchema)]
struct GetOrderRes {
    pub order: OrderEntity,
    pub order_items: Vec<CartItemEntity>,
    pub total_price: f32,
}

/// Fetch a specific order belonging to the authenticated patient.
#[utoipa::path(
    get,
    path = "/{id}",
    tags = ["Orders"],
    security(("bearerAuth" = [])),
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
    Extension(patient_id): Extension<i32>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let order: QueryResult<OrderEntity> = orders::table
        .find(id)
        // .filter(orders::deleted_at.is_null())
        .filter(orders::patient_id.eq(patient_id))
        .get_result(conn)
        .await;

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
    path = "/my-orders",
    tags = ["Orders"],
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "List my orders", body = StdResponse<Vec<GetOrderRes>, String>)
    )
)]
async fn get_my_orders(
    State(state): State<AppState>,
    Extension(patient_id): Extension<i32>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let orders: Vec<OrderEntity> = orders::table
        // .filter(orders::deleted_at.is_null())
        .filter(orders::patient_id.eq(patient_id))
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

#[derive(Deserialize, ToSchema)]
struct CreateOrderReq {
    delivery_address_id: i32,
    cart_id: i32,
}

/// Create a new order for the authenticated patient.
#[utoipa::path(
    post,
    path = "/",
    tags = ["Orders"],
    security(("bearerAuth" = [])),
    request_body = CreateOrderReq,
    responses(
        (status = 200, description = "Created order successfully", body = StdResponse<OrderEntity, String>)
    )
)]
async fn create_order(
    State(state): State<AppState>,
    Extension(patient_id): Extension<i32>,
    Json(body): Json<CreateOrderReq>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let delivery_address = get_delivery_address_as_value_with_ownership_check(
        state.http_client,
        body.delivery_address_id,
        patient_id,
    )
    .await
    .map_err(|_| {
        AppError::ForbiddenResource("Patient does not own this delivery address".into())
    })?;

    let order = conn
        .transaction(move |conn| {
            Box::pin(async move {
                let order = diesel::insert_into(orders::table)
                    .values(CreateOrderEntity {
                        patient_id,
                        delivery_address,
                        cart_id: body.cart_id,
                        status: "PENDING".into(),
                    })
                    .returning(OrderEntity::as_returning())
                    .get_result(conn)
                    .await
                    .context("Failed to create order")?;

                let order_items: Vec<CartItemEntity> = cart_items::table
                    .filter(cart_items::cart_id.eq(order.cart_id))
                    .get_results(conn)
                    .await
                    .context("Failed to get cart items")?;

                let order_items = order_items
                    .iter()
                    .map(|item| medbook_events::OrderItem {
                        product_id: item.product_id,
                        quantity: item.quantity,
                    })
                    .collect();

                outbox::publish(
                    conn,
                    "inventory.reserve_order".into(),
                    medbook_events::OrderRequestedEvent {
                        order_id: order.id,
                        order_items,
                    },
                )
                .await?;

                Ok::<OrderEntity, anyhow::Error>(order)
            })
        })
        .await
        .context("Transaction failed")?;

    Ok(StdResponse {
        data: Some(order),
        message: Some("Create order succesfully"),
    })
}

/// Cancel a reserved order for the authenticated patient.
#[utoipa::path(
    delete,
    path = "/{id}",
    tags = ["Orders"],
    security(("bearerAuth" = [])),
    params(
        ("id" = i32, Path, description = "Order ID to cancel")
    ),
    responses(
        (status = 200, description = "Cancelled order successfully", body = StdResponse<OrderEntity, String>)
    )
)]
async fn cancel_order(
    Path(id): Path<i32>,
    State(state): State<AppState>,
    Extension(patient_id): Extension<i32>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let cancelled_order = conn
        .transaction(move |conn| {
            Box::pin(async move {
                let cancelled_order: OrderEntity = diesel::update(orders::table.find(id))
                    .filter(orders::deleted_at.is_null())
                    .filter(orders::patient_id.eq(patient_id))
                    .filter(orders::status.eq("RESERVED"))
                    .set((
                        orders::deleted_at.eq(diesel::dsl::now),
                        orders::status.eq("CANCEL_PENDING"),
                    ))
                    .returning(OrderEntity::as_returning())
                    .get_result(conn)
                    .await
                    .map_err(|_| AppError::NotFound)?;

                let order_items: Vec<CartItemEntity> = cart_items::table
                    .filter(cart_items::cart_id.eq(cancelled_order.cart_id))
                    .get_results(conn)
                    .await
                    .context("Failed to get cart items")?;

                let order_items = order_items
                    .iter()
                    .map(|item| medbook_events::OrderItem {
                        product_id: item.product_id,
                        quantity: item.quantity,
                    })
                    .collect();

                outbox::publish(
                    conn,
                    "inventory.cancel_order".into(),
                    OrderCancelledEvent {
                        order_id: cancelled_order.id,
                        order_items,
                    },
                )
                .await?;

                Ok::<OrderEntity, AppError>(cancelled_order)
            })
        })
        .await?;

    Ok(StdResponse {
        data: Some(cancelled_order),
        message: Some("Cancelled order successfully"),
    })
}

#[derive(Deserialize, ToSchema)]
pub struct CreatePaymentForOrderReq {
    pub provider: String,
}

#[derive(Serialize, ToSchema)]
pub struct CreatePaymentForOrderRes {
    pub payment: PaymentEntity,
    pub updated_order: OrderEntity,
}

/// Create a new payment for an existing order.
#[utoipa::path(
    post,
    path = "/{id}/payment",
    tags = ["Orders"],
    security(("bearerAuth" = [])),
    params(
        ("id" = i32, Path, description = "Order ID to create payment for")
    ),
    request_body = CreatePaymentForOrderReq,
    responses(
        (status = 200, description = "Created payment successfully", body = StdResponse<CreatePaymentForOrderRes, String>)
    )
)]
async fn create_payment_for_order(
    Path(id): Path<i32>,
    State(state): State<AppState>,
    Extension(patient_id): Extension<i32>,
    Json(body): Json<CreatePaymentForOrderReq>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    match body.provider.as_str() {
        "qr_payment" => {}
        _ => {
            return Err(AppError::BadRequest(format!(
                "{} is not a valid payment provider",
                body.provider
            )));
        }
    }

    let order: OrderEntity = orders::table
        .find(id)
        .filter(orders::patient_id.eq(patient_id))
        .filter(orders::status.eq("RESERVED"))
        .get_result(conn)
        .await
        .map_err(|_| AppError::NotFound)?;

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

    let (updated_order, payment) = conn
        .transaction(move |conn| {
            Box::pin(async move {
                let updated_order = diesel::update(
                    orders::table
                        .find(id)
                        .filter(orders::patient_id.eq(patient_id))
                        .filter(orders::status.eq("RESERVED")),
                )
                .set(orders::status.eq("PAYMENT_PENDING"))
                .returning(OrderEntity::as_returning())
                .get_result(conn)
                .await
                .context("Failed to update order")?;

                let payment = diesel::insert_into(payments::table)
                    .values(CreatePaymentEntity {
                        order_id: updated_order.id,
                        amount: total_price,
                        provider: body.provider,
                        status: "PENDING".into(),
                    })
                    .returning(PaymentEntity::as_returning())
                    .get_result(conn)
                    .await
                    .context("Failed to create payment")?;

                Ok::<(OrderEntity, PaymentEntity), AppError>((updated_order, payment))
            })
        })
        .await
        .context("Transaction failed")?;

    Ok(StdResponse {
        data: Some(CreatePaymentForOrderRes {
            payment,
            updated_order,
        }),
        message: Some("Created payment successfully"),
    })
}
