UPDATE settings SET day_rollover_hour = 0 WHERE day_rollover_hour = 4;

PRAGMA user_version = 8;
