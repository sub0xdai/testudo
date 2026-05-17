-- Drop triggers first
DROP TRIGGER IF EXISTS response_notify ON request_responses;
DROP TRIGGER IF EXISTS queue_database_notify ON queue_database;
DROP TRIGGER IF EXISTS queue_users_notify ON queue_users;
DROP TRIGGER IF EXISTS queue_orders_notify ON queue_orders;

-- Drop functions
DROP FUNCTION IF EXISTS notify_response();
DROP FUNCTION IF EXISTS notify_queue_database();
DROP FUNCTION IF EXISTS notify_queue_users();
DROP FUNCTION IF EXISTS notify_queue_orders();

-- Drop tables
DROP TABLE IF EXISTS request_responses;
DROP TABLE IF EXISTS cache_entries;
DROP TABLE IF EXISTS queue_database;
DROP TABLE IF EXISTS queue_users;
DROP TABLE IF EXISTS queue_orders;
