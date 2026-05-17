-- Revert RSK-03 coach schema.

DROP INDEX IF EXISTS idx_coach_reports_user_generated;
DROP TABLE IF EXISTS coach_reports;
ALTER TABLE users DROP COLUMN IF EXISTS coach_banner_last_viewed_at;
ALTER TABLE users DROP COLUMN IF EXISTS coach_enabled;
