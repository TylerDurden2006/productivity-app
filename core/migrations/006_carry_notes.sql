ALTER TABLE tasks ADD COLUMN first_planned_for TEXT;
UPDATE tasks SET first_planned_for = planned_for WHERE planned_for IS NOT NULL;

ALTER TABLE goals ADD COLUMN first_period_start TEXT;
UPDATE goals SET first_period_start = period_start WHERE period_start IS NOT NULL;
ALTER TABLE goals ADD COLUMN carry_count INTEGER NOT NULL DEFAULT 0;
CREATE INDEX ix_goals_open_period ON goals (horizon_kind, period_start, band, position)
  WHERE status = 'OPEN';

ALTER TABLE horizon_notes ADD COLUMN body_markdown TEXT NOT NULL DEFAULT '';

ALTER TABLE settings ADD COLUMN auto_carry_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE settings ADD COLUMN dashboard_default_range TEXT NOT NULL DEFAULT 'WEEK';
ALTER TABLE settings ADD COLUMN notes_autosave INTEGER NOT NULL DEFAULT 1;
ALTER TABLE settings ADD COLUMN notify_block_end INTEGER NOT NULL DEFAULT 1;
ALTER TABLE settings ADD COLUMN planning_reminder_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN planning_reminder_time TEXT;
ALTER TABLE settings ADD COLUMN reduced_motion INTEGER NOT NULL DEFAULT 0;

PRAGMA user_version = 6;
