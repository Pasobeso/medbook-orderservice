use anyhow::Context;
use axum::{
    Router,
    extract::{Path, State},
    response::IntoResponse,
    routing,
};
use diesel::{ExpressionMethods, QueryDsl, SelectableHelper};
use diesel_async::{AsyncConnection, RunQueryDsl};
use medbook_core::{
    app_error::{AppError, StdResponse},
    app_state::AppState,
    outbox,
};
use medbook_events::DeliveryOrderRequestEvent;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use uuid::Uuid;

use crate::{
    models::{OrderEntity, PaymentEntity},
    schema::{
        orders::{self},
        payments,
    },
};

/// Defines all patient-facing order routes (CRUD operations + authorization).
#[deprecated]
pub fn routes() -> Router<AppState> {
    Router::new().nest(
        "/payments",
        Router::new().route("/{id}/mock-pay", routing::patch(mock_pay)),
    )
}

/// Defines routes with OpenAPI specs. Should be used over `routes()` where possible.
pub fn routes_with_openapi() -> OpenApiRouter<AppState> {
    utoipa_axum::router::OpenApiRouter::new().nest(
        "/payments",
        OpenApiRouter::new().routes(utoipa_axum::routes!(mock_pay)),
    )
}

#[derive(Serialize, ToSchema)]
pub struct MockPayRes {
    updated_payment: PaymentEntity,
    updated_order: OrderEntity,
}

/// Mock payment operation for demonstration purposes.
#[utoipa::path(
    post,
    path = "/{id}/mock-pay",
    tags = ["Payments"],
    params(
        ("id" = Uuid, Path, description = "Payment ID to mark as paid")
    ),
    responses(
        (status = 200, description = "Payment successfully marked as paid", body = StdResponse<MockPayRes, String>)
    )
)]
pub async fn mock_pay(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let conn = &mut state
        .db_pool
        .get()
        .await
        .context("Failed to obtain a DB connection pool")?;

    let (updated_payment, updated_order) = conn
        .transaction(move |conn| {
            Box::pin(async move {
                let updated_payment = diesel::update(
                    payments::table
                        .find(id)
                        .filter(payments::status.eq("PENDING")),
                )
                .set(payments::status.eq("PAID"))
                .returning(PaymentEntity::as_returning())
                .get_result(conn)
                .await
                .context("Failed to update payment status")?;

                let updated_order = diesel::update(
                    orders::table
                        .find(updated_payment.order_id)
                        .filter(orders::status.eq("PAYMENT_PENDING")),
                )
                .set(orders::status.eq("DELIVERY_PENDING"))
                .returning(OrderEntity::as_returning())
                .get_result(conn)
                .await
                .context("Failed to update order status")?;

                outbox::publish(
                    conn,
                    "delivery.order_request".into(),
                    DeliveryOrderRequestEvent {
                        delivery_address: updated_order.delivery_address.clone(),
                        order_id: updated_order.id.clone(),
                        order_type: updated_order.order_type.clone(),
                    },
                )
                .await
                .context("Failed to send outbox")?;

                Ok::<(PaymentEntity, OrderEntity), AppError>((updated_payment, updated_order))
            })
        })
        .await
        .context("Transaction failed")?;

    Ok(StdResponse {
        data: Some(MockPayRes {
            updated_order,
            updated_payment,
        }),
        message: Some("Payment paid successfully"),
    })
}
