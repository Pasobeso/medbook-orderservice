-- Your SQL goes here
CREATE TABLE "outbox" (
  "id" serial PRIMARY KEY,
  "event_type" text NOT NULL,
  "payload" text NOT NULL,
  "status" text NOT NULL DEFAULT 'PENDING',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER update_outbox_timestamp
BEFORE UPDATE ON outbox
FOR EACH ROW
EXECUTE FUNCTION diesel_set_updated_at();
