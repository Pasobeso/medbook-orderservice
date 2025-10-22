-- Your SQL goes here

CREATE TABLE "carts"(
  id SERIAL PRIMARY KEY,
  patient_id INTEGER NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER update_carts_timestamp
BEFORE UPDATE ON carts
FOR EACH ROW
EXECUTE FUNCTION diesel_set_updated_at();

CREATE TABLE "cart_items" (
    cart_id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (cart_id) REFERENCES carts(id) ON DELETE CASCADE,
    PRIMARY KEY ("cart_id", "product_id")
);

CREATE TRIGGER update_cart_items_timestamp
BEFORE UPDATE ON cart_items
FOR EACH ROW
EXECUTE FUNCTION diesel_set_updated_at();

CREATE TABLE "orders" (
  "id" serial PRIMARY KEY,
  "cart_id" integer NOT NULL,
  "patient_id" integer NOT NULL,
  "status" text NOT NULL DEFAULT 'PENDING',
  "order_type" text NOT NULL DEFAULT 'PICKUP', -- PICKUP, DELIVERY,
  "delivery_id" UUID,
  "delivery_address" JSONB,
  "created_at" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updated_at" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deleted_at" TIMESTAMPTZ,
  FOREIGN KEY (cart_id) REFERENCES carts(id) ON DELETE CASCADE
);


CREATE TRIGGER update_orders_timestamp
BEFORE UPDATE ON orders
FOR EACH ROW
EXECUTE FUNCTION diesel_set_updated_at();

CREATE TABLE payments (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id          INTEGER NOT NULL,
    amount            REAL NOT NULL,
    status            VARCHAR(32) NOT NULL DEFAULT 'PENDING', -- PENDING, SUCCESS, FAILED
    provider          VARCHAR(64) NOT NULL DEFAULT 'internal', -- PromptPay, etc.
    provider_ref      VARCHAR(128),  -- external transaction reference
    failure_reason    TEXT,          -- for failed payments
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE
);

CREATE TRIGGER update_payments_timestamp
BEFORE UPDATE ON payments
FOR EACH ROW
EXECUTE FUNCTION diesel_set_updated_at();
