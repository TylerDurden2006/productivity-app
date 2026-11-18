CREATE TABLE habits (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  why TEXT,
  cue TEXT,
  area_id TEXT REFERENCES areas(id),
  schedule_json TEXT NOT NULL,
  anchor_on TEXT NOT NULL,
  target_count INTEGER NOT NULL DEFAULT 1 CHECK (target_count >= 1),
  unit_label TEXT,
  time_of_day TEXT NOT NULL DEFAULT 'ANYTIME'
    CHECK (time_of_day IN ('MORNING', 'AFTERNOON', 'EVENING', 'ANYTIME')),
  sort_order INTEGER NOT NULL DEFAULT 0,
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  starts_on TEXT,
  ends_on TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX ix_habits_active ON habits (active, sort_order);

CREATE TABLE habit_entries (
  id TEXT PRIMARY KEY,
  habit_id TEXT NOT NULL REFERENCES habits(id) ON DELETE CASCADE,
  on_date TEXT NOT NULL,
  count INTEGER NOT NULL DEFAULT 1 CHECK (count >= 0),
  status TEXT NOT NULL DEFAULT 'DONE' CHECK (status IN ('DONE', 'SKIPPED')),
  note TEXT,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (habit_id, on_date)
);
CREATE INDEX ix_habit_entries_date ON habit_entries (on_date, status);
