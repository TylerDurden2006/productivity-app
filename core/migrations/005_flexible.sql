ALTER TABLE recurrences ADD COLUMN schedule_json TEXT;
ALTER TABLE recurrences ADD COLUMN anchor_on TEXT;
UPDATE recurrences SET anchor_on = starts_on WHERE anchor_on IS NULL;
UPDATE recurrences
SET schedule_json = '{"kind":"DAILY"}'
WHERE freq = 'DAILY' AND schedule_json IS NULL;
UPDATE recurrences
SET schedule_json = '{"kind":"WEEKLY","weekdays_mask":' || weekdays_mask || '}'
WHERE freq = 'WEEKLY' AND schedule_json IS NULL;
UPDATE recurrences
SET schedule_json = '{"kind":"EVERY_N_DAYS","interval_n":' || interval_n || '}'
WHERE freq = 'EVERY_N_DAYS' AND schedule_json IS NULL;
