-- ENG-01c FR-6: opt-in streak visibility on public profile.
-- Default FALSE — matches existing opt-in-per-element privacy model.

ALTER TABLE user_handles
    ADD COLUMN show_streak BOOLEAN NOT NULL DEFAULT FALSE;
