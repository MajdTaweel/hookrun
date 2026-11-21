CREATE TABLE idempotency_keys (
    key text PRIMARY KEY,
    delivery_id bigint NOT NULL REFERENCES deliveries(id)
);
