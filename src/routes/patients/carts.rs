use std::collections::HashMap;

use anyhow::{Context, Result};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing,
};
use diesel::{ExpressionMethods, QueryDsl, QueryResult, SelectableHelper};
use diesel_async::{AsyncConnection, RunQueryDsl};
use medbook_core::{
    aliases::DieselError,
    app_error::{AppError, StdResponse},
    app_state::AppState,
    middleware::{self},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;

use crate::{
    api::products::get_product_unit_prices,
    models::{CartEntity, CartItemEntity, CreateCartEntity, CreateCartItemEntity},
    schema::{
        cart_items::{self},
        carts,
    },
};

/// Defines all patient-facing carts routes (CRUD operations + authorization).
#[deprecated]
pub fn routes() -> Router<AppState> {
    Router::new().nest(
        "/patients/carts",
        Router::new()
            .route("/", routing::get(get_carts))
            .route("/", routing::post(create_cart))
            .route("/{id}", routing::patch(update_cart))
            .route("/{id}", routing::get(get_cart))
            .route("/{id}", routing::delete(delete_cart))
            .route("/my-carts", routing::get(get_my_carts))
            .route_layer(axum::middleware::from_fn(
                middleware::patients_authorization,
            )),
    )
}

/// Defines routes with OpenAPI specs. Should be used over `routes()` where possible.
pub fn routes_with_openapi() -> OpenApiRouter<AppState> {
    utoipa_axum::router::OpenApiRouter::new().nest(
        "/patients/carts",
        OpenApiRouter::new()
            .routes(utoipa_axum::routes!(get_carts))
            .routes(utoipa_axum::routes!(get_cart))
            .routes(utoipa_axum::routes!(get_my_carts))
            .routes(utoipa_axum::routes!(delete_cart))
            .routes(utoipa_axum::routes!(create_cart))
            .routes(utoipa_axum::routes!(update_cart))
            .route_layer(axum::middleware::from_fn(
                middleware::patients_authorization,
            )),
    )
}

/// Get all carts in the system (admin or debugging use).
#[utoipa::path(
    get,
    path = "/",
    tags = ["Carts"],
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "List all carts", body = StdResponse<Vec<CartEntity>, String>)
    )
)]
async fn get_carts(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let carts: Vec<CartEntity> = carts::table
        .get_results(conn)
        .await
        .context("Failed to get carts")?;

    Ok(StdResponse {
        data: Some(carts),
        message: Some("Get carts successfully"),
    })
}

#[derive(Serialize, ToSchema)]
struct GetCartRes {
    pub cart: CartEntity,
    pub cart_items: Vec<CartItemEntity>,
    pub total_price: f32,
}

/// Get a specific cart belonging to the authenticated patient.
#[utoipa::path(
    get,
    path = "/{id}",
    tags = ["Carts"],
    security(("bearerAuth" = [])),
    params(
        ("id" = i32, Path, description = "Cart ID to fetch")
    ),
    responses(
        (status = 200, description = "Get cart successfully", body = StdResponse<GetCartRes, String>)
    )
)]
async fn get_cart(
    Path(id): Path<i32>,
    State(state): State<AppState>,
    Extension(patient_id): Extension<i32>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let cart: QueryResult<CartEntity> = carts::table
        .find(id)
        .filter(carts::patient_id.eq(patient_id))
        .get_result(conn)
        .await;

    if let Err(err) = cart {
        match err {
            DieselError::NotFound => return Err(AppError::NotFound),
            _ => return Err(AppError::Other(err.into())),
        }
    }

    let cart = cart.unwrap();

    let cart_items: Vec<CartItemEntity> = cart_items::table
        .filter(cart_items::cart_id.eq(cart.id))
        .get_results(conn)
        .await
        .context("Failed to get cart items")?;

    let cart_item_ids = cart_items.iter().map(|item| item.product_id).collect();
    let unit_prices = get_product_unit_prices(state.http_client, cart_item_ids).await?;

    let total_price: f32 = cart_items
        .iter()
        .map(|item| {
            let unit_price: f32 = unit_prices.get(&item.product_id).copied().unwrap_or(0.0);
            item.quantity as f32 * unit_price
        })
        .sum();

    Ok(StdResponse {
        data: Some(GetCartRes {
            cart,
            cart_items,
            total_price,
        }),
        message: Some("Get cart successfully"),
    })
}

/// Get all carts belonging to the current authenticated patient.
#[utoipa::path(
    get,
    path = "/my-carts",
    tags = ["Carts"],
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "List my carts", body = StdResponse<Vec<GetCartRes>, String>)
    )
)]
async fn get_my_carts(
    State(state): State<AppState>,
    Extension(patient_id): Extension<i32>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let carts: Vec<CartEntity> = carts::table
        .filter(carts::patient_id.eq(patient_id))
        .get_results(conn)
        .await
        .context("Failed to get my carts")?;

    let cart_ids: Vec<i32> = carts.iter().map(|cart| cart.id).collect();

    let cart_items: Vec<CartItemEntity> = cart_items::table
        .filter(cart_items::cart_id.eq_any(&cart_ids))
        .get_results(conn)
        .await
        .context("Failed to get cart items")?;

    let cart_item_ids = cart_items.iter().map(|item| item.product_id).collect();
    let unit_prices = get_product_unit_prices(state.http_client, cart_item_ids).await?;

    let mut group: HashMap<i32, Vec<CartItemEntity>> = HashMap::new();
    for item in cart_items {
        group.entry(item.cart_id).or_default().push(item);
    }

    let carts_with_items: Vec<GetCartRes> = carts
        .into_iter()
        .map(|cart| {
            let cart_items = group.remove(&cart.id).unwrap_or_default();
            let total_price = cart_items
                .iter()
                .map(|item| {
                    let unit_price = unit_prices.get(&item.product_id).copied().unwrap_or(0.0);
                    item.quantity as f32 * unit_price
                })
                .sum();
            GetCartRes {
                cart_items,
                cart,
                total_price,
            }
        })
        .collect();

    Ok(StdResponse {
        data: Some(carts_with_items),
        message: Some("Get my carts successfully"),
    })
}

/// Delete a cart belonging to the authenticated patient.
#[utoipa::path(
    delete,
    path = "/{id}",
    tags = ["Carts"],
    security(("bearerAuth" = [])),
    params(
        ("id" = i32, Path, description = "Cart ID to delete")
    ),
    responses(
        (status = 200, description = "Deleted cart successfully", body = StdResponse<CartEntity, String>)
    )
)]
async fn delete_cart(
    Path(id): Path<i32>,
    State(state): State<AppState>,
    Extension(patient_id): Extension<i32>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let cart: QueryResult<CartEntity> = diesel::delete(carts::table)
        .filter(carts::id.eq(id))
        .filter(carts::patient_id.eq(patient_id))
        .returning(CartEntity::as_returning())
        .get_result(conn)
        .await;

    match cart {
        Ok(cart) => Ok(StdResponse {
            data: Some(cart),
            message: Some("Deleted cart successfully"),
        }),
        Err(err) => match err {
            DieselError::NotFound => Err(AppError::NotFound),
            _ => Err(AppError::Other(err.into())),
        },
    }
}

/// Create a new cart for the patient.

#[derive(Deserialize, ToSchema)]
struct CreateCartReq {
    pub cart_items: Vec<CreateCartReqCartItem>,
}

#[derive(Deserialize, ToSchema)]
struct CreateCartReqCartItem {
    pub product_id: i32,
    pub quantity: i32,
}

#[derive(Serialize, ToSchema)]
struct CreateCartRes {
    pub cart: CartEntity,
    pub cart_items: Vec<CartItemEntity>,
}

/// Create a new cart for the authenticated patient.
#[utoipa::path(
    post,
    path = "/",
    tags = ["Carts"],
    security(("bearerAuth" = [])),
    request_body = CreateCartReq,
    responses(
        (status = 200, description = "Created cart successfully", body = StdResponse<CreateCartRes, String>)
    )
)]
async fn create_cart(
    State(state): State<AppState>,
    Extension(patient_id): Extension<i32>,
    Json(body): Json<CreateCartReq>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let (cart, cart_items) = conn
        .transaction(move |tx| {
            Box::pin(async move {
                let cart: CartEntity = diesel::insert_into(carts::table)
                    .values(CreateCartEntity { patient_id })
                    .returning(CartEntity::as_returning())
                    .get_result(tx)
                    .await
                    .context("Failed to create cart")?;

                let cart_items: Vec<CreateCartItemEntity> = body
                    .cart_items
                    .into_iter()
                    .filter(|item| item.quantity > 0)
                    .map(|item| CreateCartItemEntity {
                        cart_id: cart.id,
                        product_id: item.product_id,
                        quantity: item.quantity,
                    })
                    .collect();

                let cart_items = diesel::insert_into(cart_items::table)
                    .values(cart_items)
                    .returning(CartItemEntity::as_returning())
                    .get_results(tx)
                    .await
                    .context("Failed to create cart items")?;

                Ok::<(CartEntity, Vec<CartItemEntity>), anyhow::Error>((cart, cart_items))
            })
        })
        .await
        .context("Transaction failed")?;

    Ok(StdResponse {
        data: Some(CreateCartRes { cart, cart_items }),
        message: Some("Created cart successfully"),
    })
}

/// Update a cart

#[derive(Serialize, ToSchema)]
struct UpdateCartRes {
    pub deleted_items: Vec<CartItemEntity>,
    pub updated_items: Vec<CartItemEntity>,
    pub updated_cart: CartEntity,
}

/// Update a specific cart’s contents for the authenticated patient.
#[utoipa::path(
    patch,
    path = "/{id}",
    tags = ["Carts"],
    security(("bearerAuth" = [])),
    params(
        ("id" = i32, Path, description = "Cart ID to update")
    ),
    request_body = CreateCartReq,
    responses(
        (status = 200, description = "Updated cart successfully", body = StdResponse<UpdateCartRes, String>)
    )
)]
async fn update_cart(
    Path(id): Path<i32>,
    State(state): State<AppState>,
    Extension(patient_id): Extension<i32>,
    Json(body): Json<CreateCartReq>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let result = conn
        .transaction(move |conn| {
            Box::pin(async move {
                let cart: i64 = carts::table
                    .find(id)
                    .filter(carts::patient_id.eq(patient_id))
                    .count()
                    .get_result(conn)
                    .await
                    .context("Failed to get count")?;

                if cart == 0 {
                    return Err(AppError::NotFound);
                }

                let new_product_ids: Vec<i32> =
                    body.cart_items.iter().map(|item| item.product_id).collect();

                let deleted_items: Vec<CartItemEntity> = diesel::delete(
                    cart_items::table
                        .filter(cart_items::cart_id.eq(id))
                        .filter(cart_items::product_id.ne_all(&new_product_ids)),
                )
                .returning(CartItemEntity::as_returning())
                .get_results(conn)
                .await
                .context("Failed to delete cart items")?;

                for item in &body.cart_items {
                    diesel::insert_into(cart_items::table)
                        .values((
                            cart_items::cart_id.eq(id),
                            cart_items::product_id.eq(item.product_id),
                            cart_items::quantity.eq(item.quantity),
                        ))
                        .on_conflict((cart_items::cart_id, cart_items::product_id))
                        .do_update()
                        .set(cart_items::quantity.eq(item.quantity))
                        .execute(conn)
                        .await
                        .context("Failed to upsert cart item")?;
                }

                let updated_cart = diesel::update(carts::table.find(id))
                    .set(carts::updated_at.eq(diesel::dsl::now))
                    .returning(CartEntity::as_returning())
                    .get_result(conn)
                    .await
                    .context("Failed to update cart timestamp")?;

                let updated_items: Vec<CartItemEntity> = cart_items::table
                    .filter(cart_items::cart_id.eq(id))
                    .get_results(conn)
                    .await
                    .context("Failed to get updated items")?;

                Ok::<(Vec<CartItemEntity>, Vec<CartItemEntity>, CartEntity), AppError>((
                    deleted_items,
                    updated_items,
                    updated_cart,
                ))
            })
        })
        .await;

    match result {
        Ok((deleted_items, updated_items, updated_cart)) => Ok(StdResponse {
            data: Some(UpdateCartRes {
                deleted_items,
                updated_items,
                updated_cart,
            }),
            message: Some("Updated cart successfully"),
        }),
        Err(err) => Err(err.into()),
    }
}
