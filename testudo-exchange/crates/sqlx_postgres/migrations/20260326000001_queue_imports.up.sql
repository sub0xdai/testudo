-- HIST-01: Import job queue table (same pattern as queue_orders)

CREATE TABLE IF NOT EXISTS queue_imports (
    id BIGSERIAL PRIMARY KEY,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_queue_imports_pending ON queue_imports(created_at) WHERE status = 'pending';

CREATE OR REPLACE FUNCTION notify_queue_imports() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('queue_imports', NEW.id::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER queue_imports_notify
    AFTER INSERT ON queue_imports
    FOR EACH ROW EXECUTE FUNCTION notify_queue_imports();
