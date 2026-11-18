CREATE TABLE try_items (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  notes TEXT,
  area_id TEXT REFERENCES areas(id),
  category TEXT,
  link TEXT,
  effort TEXT NOT NULL DEFAULT 'MEDIUM' CHECK (effort IN ('LOW', 'MEDIUM', 'HIGH')),
  cost TEXT NOT NULL DEFAULT 'LOW' CHECK (cost IN ('FREE', 'LOW', 'MEDIUM', 'HIGH')),
  social_mode TEXT NOT NULL DEFAULT 'EITHER' CHECK (social_mode IN ('SOLO', 'SOCIAL', 'EITHER')),
  energy TEXT NOT NULL DEFAULT 'MEDIUM' CHECK (energy IN ('LOW', 'MEDIUM', 'HIGH')),
  status TEXT NOT NULL DEFAULT 'IDEA' CHECK (status IN ('IDEA', 'SCHEDULED', 'TRIED', 'SKIPPED')),
  scheduled_for TEXT,
  scheduled_day_part TEXT
    CHECK (scheduled_day_part IS NULL OR scheduled_day_part IN ('MORNING', 'AFTERNOON', 'EVENING', 'ANYTIME')),
  linked_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  rating INTEGER CHECK (rating BETWEEN 1 AND 5),
  reflection TEXT,
  tried_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (status <> 'SCHEDULED' OR scheduled_for IS NOT NULL)
);
CREATE INDEX ix_try_status ON try_items (status);
CREATE INDEX ix_try_scheduled ON try_items (scheduled_for) WHERE scheduled_for IS NOT NULL;
