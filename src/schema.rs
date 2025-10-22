// @generated automatically by Diesel CLI.

diesel::table! {
    cart_items (cart_id, product_id) {
        cart_id -> Int4,
        product_id -> Int4,
        quantity -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    carts (id) {
        id -> Int4,
        patient_id -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    orders (id) {
        id -> Int4,
        cart_id -> Int4,
        patient_id -> Int4,
        status -> Text,
        order_type -> Text,
        delivery_id -> Nullable<Uuid>,
        delivery_address -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    outbox (id) {
        id -> Int4,
        event_type -> Text,
        payload -> Text,
        status -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    payments (id) {
        id -> Uuid,
        order_id -> Int4,
        amount -> Float4,
        #[max_length = 32]
        status -> Varchar,
        #[max_length = 64]
        provider -> Varchar,
        #[max_length = 128]
        provider_ref -> Nullable<Varchar>,
        failure_reason -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::joinable!(cart_items -> carts (cart_id));
diesel::joinable!(orders -> carts (cart_id));
diesel::joinable!(payments -> orders (order_id));

diesel::allow_tables_to_appear_in_same_query!(cart_items, carts, orders, outbox, payments,);
