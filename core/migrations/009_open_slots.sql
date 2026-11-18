PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE tasks_new (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  first_step TEXT,
  done_when TEXT,
  estimate_mins INTEGER CHECK (estimate_mins IN (15, 30, 60, 90, 120)),
  status TEXT NOT NULL DEFAULT 'INBOX'
    CHECK (status IN ('INBOX', 'READY', 'WAITING', 'SOMEDAY', 'DONE', 'DROPPED')),
  value INTEGER NOT NULL DEFAULT 3 CHECK (value BETWEEN 1 AND 5),
  pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
  deadline_on TEXT,
  waiting_on TEXT,
  review_on TEXT,
  breadcrumb TEXT,
  cue TEXT,
  parent_id TEXT REFERENCES tasks(id),
  closed_reason TEXT CHECK (closed_reason IN ('SPLIT', 'NOT_DOING')),
  rollover_count INTEGER NOT NULL DEFAULT 0 CHECK (rollover_count >= 0),
  planned_for TEXT,
  slot TEXT CHECK (slot IN ('ONE', 'THREE', 'BATCH', 'EXTRA')),
  slot_pos INTEGER,
  recurrence_id TEXT REFERENCES recurrences(id),
  occurrence_on TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  first_planned_for TEXT,
  CHECK ((planned_for IS NULL) = (slot IS NULL) AND (slot IS NULL) = (slot_pos IS NULL)),
  CHECK (slot IS NULL OR status IN ('READY', 'DONE')),
  CHECK (slot IS NULL
    OR status = 'DONE'
    OR (slot = 'ONE' AND slot_pos = 1)
    OR (slot = 'THREE' AND slot_pos BETWEEN 1 AND 3)
    OR (slot = 'BATCH' AND slot_pos BETWEEN 1 AND 5)
    OR (slot = 'EXTRA' AND slot_pos >= 1)),
  CHECK (status <> 'READY' OR (first_step IS NOT NULL AND length(trim(first_step)) >= 3
                                AND estimate_mins IS NOT NULL)),
  CHECK ((recurrence_id IS NULL) = (occurrence_on IS NULL)),
  CHECK (recurrence_id IS NULL OR status IN ('READY', 'DONE', 'DROPPED'))
);

INSERT INTO tasks_new (
  id, title, first_step, done_when, estimate_mins, status, value, pinned,
  deadline_on, waiting_on, review_on, breadcrumb, cue, parent_id, closed_reason,
  rollover_count, planned_for, slot, slot_pos, recurrence_id, occurrence_on,
  created_at, updated_at, completed_at, first_planned_for
)
SELECT
  id, title, first_step, done_when, estimate_mins, status, value, pinned,
  deadline_on, waiting_on, review_on, breadcrumb, cue, parent_id, closed_reason,
  rollover_count, planned_for, slot, slot_pos, recurrence_id, occurrence_on,
  created_at, updated_at, completed_at, first_planned_for
FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE UNIQUE INDEX ux_slot_pos ON tasks (planned_for, slot, slot_pos)
  WHERE slot IS NOT NULL AND status = 'READY';
CREATE INDEX ix_tasks_status ON tasks (status);
CREATE INDEX ix_tasks_review ON tasks (review_on) WHERE review_on IS NOT NULL;
CREATE INDEX ix_tasks_planned ON tasks (planned_for) WHERE planned_for IS NOT NULL;
CREATE INDEX ix_tasks_recurrence ON tasks (recurrence_id) WHERE recurrence_id IS NOT NULL;
CREATE UNIQUE INDEX ux_recurrence_occurrence ON tasks (recurrence_id, occurrence_on)
  WHERE recurrence_id IS NOT NULL;
CREATE UNIQUE INDEX ux_one_open_recurrence ON tasks (recurrence_id)
  WHERE recurrence_id IS NOT NULL AND status = 'READY';

COMMIT;

PRAGMA foreign_keys = ON;

PRAGMA user_version = 9;
