CREATE TABLE settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  day_rollover_hour INTEGER NOT NULL DEFAULT 0 CHECK (day_rollover_hour BETWEEN 0 AND 23),
  daily_focus_budget_mins INTEGER NOT NULL DEFAULT 240 CHECK (daily_focus_budget_mins > 0),
  ready_cap INTEGER NOT NULL DEFAULT 10 CHECK (ready_cap >= 0),
  focus_block_mins INTEGER NOT NULL DEFAULT 90 CHECK (focus_block_mins > 0),
  batch_block_mins INTEGER NOT NULL DEFAULT 45 CHECK (batch_block_mins > 0),
  default_multiplier REAL NOT NULL DEFAULT 1.25 CHECK (default_multiplier > 0),
  last_weekly_review_at TEXT
);
INSERT INTO settings (id) VALUES (1);

CREATE TABLE recurrences (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  first_step TEXT NOT NULL CHECK (length(trim(first_step)) >= 3),
  done_when TEXT,
  estimate_mins INTEGER NOT NULL CHECK (estimate_mins IN (15, 30, 60, 90, 120)),
  value INTEGER NOT NULL DEFAULT 3 CHECK (value BETWEEN 1 AND 5),
  cue TEXT,
  freq TEXT NOT NULL CHECK (freq IN ('DAILY', 'WEEKLY', 'EVERY_N_DAYS')),
  interval_n INTEGER NOT NULL DEFAULT 1 CHECK (interval_n BETWEEN 1 AND 365),
  weekdays_mask INTEGER NOT NULL DEFAULT 127 CHECK (weekdays_mask BETWEEN 1 AND 127),
  starts_on TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (freq <> 'WEEKLY' OR weekdays_mask > 0),
  CHECK (freq <> 'EVERY_N_DAYS' OR interval_n >= 1)
);

CREATE TABLE tasks (
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
  CHECK ((planned_for IS NULL) = (slot IS NULL) AND (slot IS NULL) = (slot_pos IS NULL)),
  CHECK (slot IS NULL OR status IN ('READY', 'DONE')),
  CHECK (slot IS NULL
    OR (slot = 'ONE' AND slot_pos = 1)
    OR (slot = 'THREE' AND slot_pos BETWEEN 1 AND 3)
    OR (slot = 'BATCH' AND slot_pos BETWEEN 1 AND 5)
    OR (slot = 'EXTRA' AND slot_pos >= 1)),
  CHECK (status <> 'READY' OR (first_step IS NOT NULL AND length(trim(first_step)) >= 3
                                AND estimate_mins IS NOT NULL)),
  CHECK ((recurrence_id IS NULL) = (occurrence_on IS NULL)),
  CHECK (recurrence_id IS NULL OR status IN ('READY', 'DONE', 'DROPPED'))
);
CREATE UNIQUE INDEX ux_slot_pos ON tasks (planned_for, slot, slot_pos) WHERE slot IS NOT NULL;
CREATE INDEX ix_tasks_status ON tasks (status);
CREATE INDEX ix_tasks_review ON tasks (review_on) WHERE review_on IS NOT NULL;
CREATE INDEX ix_tasks_planned ON tasks (planned_for) WHERE planned_for IS NOT NULL;
CREATE INDEX ix_tasks_recurrence ON tasks (recurrence_id) WHERE recurrence_id IS NOT NULL;
CREATE UNIQUE INDEX ux_recurrence_occurrence ON tasks (recurrence_id, occurrence_on)
  WHERE recurrence_id IS NOT NULL;
CREATE UNIQUE INDEX ux_one_open_recurrence ON tasks (recurrence_id)
  WHERE recurrence_id IS NOT NULL AND status = 'READY';

CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('FOCUS', 'BATCH', 'MANUAL')),
  task_id TEXT REFERENCES tasks(id),
  day TEXT NOT NULL,
  planned_mins INTEGER NOT NULL,
  extended INTEGER NOT NULL DEFAULT 0 CHECK (extended IN (0, 1)),
  accumulated_secs INTEGER NOT NULL DEFAULT 0 CHECK (accumulated_secs >= 0),
  running_since TEXT,
  last_seen_at TEXT,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  CHECK (kind = 'BATCH' OR task_id IS NOT NULL)
);
CREATE UNIQUE INDEX ux_one_active_session ON sessions ((ended_at IS NULL)) WHERE ended_at IS NULL;
CREATE INDEX ix_sessions_task ON sessions (task_id, day);

CREATE TABLE days (
  day TEXT PRIMARY KEY,
  plan_locked_at TEXT,
  multiplier_at_lock REAL,
  budget_at_lock INTEGER,
  wrapup_done_at TEXT,
  win_note TEXT
);

CREATE TABLE events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  at TEXT NOT NULL,
  task_id TEXT REFERENCES tasks(id),
  kind TEXT NOT NULL,
  reason TEXT,
  before_json TEXT
);
CREATE INDEX ix_events_task ON events (task_id, id);

PRAGMA user_version = 1;
