-- Queue Tables with LISTEN/NOTIFY triggers for immediate wake

-- Queue: Orders
CREATE TABLE IF NOT EXISTS queue_orders (
    id BIGSERIAL PRIMARY KEY,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_queue_orders_pending ON queue_orders(created_at) WHERE status = 'pending';

CREATE OR REPLACE FUNCTION notify_queue_orders() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('queue_orders', NEW.id::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER queue_orders_notify
    AFTER INSERT ON queue_orders
    FOR EACH ROW EXECUTE FUNCTION notify_queue_orders();

-- Queue: Users
CREATE TABLE IF NOT EXISTS queue_users (
    id BIGSERIAL PRIMARY KEY,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_queue_users_pending ON queue_users(created_at) WHERE status = 'pending';

CREATE OR REPLACE FUNCTION notify_queue_users() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('queue_users', NEW.id::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER queue_users_notify
    AFTER INSERT ON queue_users
    FOR EACH ROW EXECUTE FUNCTION notify_queue_users();

-- Queue: Database
CREATE TABLE IF NOT EXISTS queue_database (
    id BIGSERIAL PRIMARY KEY,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_queue_database_pending ON queue_database(created_at) WHERE status = 'pending';

CREATE OR REPLACE FUNCTION notify_queue_database() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('queue_database', NEW.id::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER queue_database_notify
    AFTER INSERT ON queue_database
    FOR EACH ROW EXECUTE FUNCTION notify_queue_database();

-- Cache Table (UNLOGGED for performance, per-query TTL check)
CREATE UNLOGGED TABLE IF NOT EXISTS cache_entries (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);

-- Request-Response Table for RPC-style operations
CREATE TABLE IF NOT EXISTS request_responses (
    request_id UUID PRIMARY KEY,
    response JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION notify_response() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('response_' || NEW.request_id::text, '');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER response_notify
    AFTER INSERT ON request_responses
    FOR EACH ROW EXECUTE FUNCTION notify_response();
