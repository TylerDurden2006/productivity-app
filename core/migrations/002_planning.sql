ALTER TABLE settings ADD COLUMN week_starts_on INTEGER NOT NULL DEFAULT 0;

CREATE TABLE areas (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(trim(name)) > 0),
  color TEXT NOT NULL DEFAULT 'forest',
  sort_order INTEGER NOT NULL DEFAULT 0,
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE goals (
  id TEXT PRIMARY KEY,
  horizon_kind TEXT NOT NULL CHECK (horizon_kind IN ('WEEK', 'MONTH', 'QUARTER', 'YEAR', 'LONG_TERM')),
  period_start TEXT,
  area_id TEXT REFERENCES areas(id),
  parent_id TEXT REFERENCES goals(id),
  band TEXT NOT NULL CHECK (band IN ('FOCUS', 'SECONDARY', 'COMMITMENT', 'MORE')),
  position INTEGER NOT NULL DEFAULT 1 CHECK (position >= 1),
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  description TEXT,
  success_definition TEXT,
  status TEXT NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN', 'DONE', 'DROPPED')),
  target_on TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (
    (horizon_kind = 'LONG_TERM' AND period_start IS NULL)
    OR (horizon_kind <> 'LONG_TERM' AND period_start IS NOT NULL)
  )
);
CREATE INDEX ix_goals_horizon ON goals (horizon_kind, period_start, band, position);
CREATE INDEX ix_goals_status ON goals (status);
CREATE INDEX ix_goals_area ON goals (area_id);
CREATE INDEX ix_goals_parent ON goals (parent_id);

CREATE TABLE task_goals (
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  PRIMARY KEY (task_id, goal_id)
);
CREATE INDEX ix_task_goals_goal ON task_goals (goal_id);

CREATE TABLE horizon_notes (
  horizon_kind TEXT NOT NULL,
  period_start TEXT NOT NULL,
  intent TEXT,
  review_note TEXT,
  reviewed_at TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (horizon_kind, period_start)
);

CREATE TABLE goal_checkins (
  id TEXT PRIMARY KEY,
  goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  note TEXT,
  confidence INTEGER CHECK (confidence BETWEEN 1 AND 5),
  created_at TEXT NOT NULL
);
CREATE INDEX ix_goal_checkins_goal ON goal_checkins (goal_id, created_at);
