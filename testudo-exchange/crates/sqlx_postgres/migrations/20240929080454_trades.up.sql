-- Add up migration script here
CREATE TABLE IF NOT EXISTS trades (
    trade_id BIGINT PRIMARY KEY,
    market VARCHAR NOT NULL,
    price NUMERIC NOT NULL,
    quantity NUMERIC NOT NULL,
    user_id VARCHAR NOT NULL,
    other_user_id VARCHAR NOT NULL,
    order_id VARCHAR NOT NULL,
    timestamp BIGINT NOT NULL
);
