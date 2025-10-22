use chrono::{DateTime, Utc};
use diesel::{
    Selectable,
    prelude::{Identifiable, Insertable, Queryable},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

// Carts

#[derive(Queryable, Selectable, Identifiable, Serialize, Debug, ToSchema)]
#[diesel(table_name = crate::schema::carts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CartEntity {
    pub id: i32,
    pub patient_id: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Queryable, Selectable, Serialize, Debug, ToSchema)]
#[diesel(belongs_to(CartEntity, foreign_key = cart_id))]
#[diesel(table_name = crate::schema::cart_items)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CartItemEntity {
    pub cart_id: i32,
    pub product_id: i32,
    pub quantity: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = crate::schema::carts)]
pub struct CreateCartEntity {
    pub patient_id: i32,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = crate::schema::cart_items)]
pub struct CreateCartItemEntity {
    pub cart_id: i32,
    pub product_id: i32,
    pub quantity: i32,
}

// Orders

#[derive(Queryable, Serialize, Selectable, Debug, ToSchema)]
#[diesel(table_name = crate::schema::orders)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OrderEntity {
    pub id: i32,
    pub cart_id: i32,
    pub patient_id: i32,
    pub status: String,
    pub order_type: String,
    pub delivery_id: Option<Uuid>,
    pub delivery_address: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::orders)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CreateOrderEntity {
    pub patient_id: i32,
    pub delivery_address: Option<Value>,
    pub cart_id: i32,
    pub status: String,
    pub order_type: String,
}

#[derive(Queryable, Serialize, Selectable, Debug, Clone, ToSchema)]
#[diesel(table_name = crate::schema::payments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PaymentEntity {
    pub id: Uuid,
    pub order_id: i32,
    pub amount: f32,
    pub status: String,
    pub provider: String,
    pub provider_ref: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[diesel(table_name = crate::schema::payments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CreatePaymentEntity {
    pub order_id: i32,
    pub amount: f32,
    pub provider: String,
    pub status: String,
}
