ALTER TABLE goals ADD COLUMN estimate_mins INTEGER
  CHECK (estimate_mins IS NULL OR estimate_mins IN (15, 30, 60, 90, 120));

PRAGMA user_version = 7;
