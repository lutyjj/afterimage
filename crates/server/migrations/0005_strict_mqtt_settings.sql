-- mqtt_settings as STRICT as every other table, so that a value of another type
-- is refused rather than stored.
CREATE TABLE mqtt_settings_strict (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    url              TEXT    NOT NULL,
    username         TEXT,
    password         TEXT,
    topic            TEXT    NOT NULL,
    discovery_prefix TEXT    NOT NULL,
    updated_at       INTEGER NOT NULL
) STRICT;
INSERT INTO mqtt_settings_strict
    SELECT id, url, username, password, topic, discovery_prefix, updated_at FROM mqtt_settings;
DROP TABLE mqtt_settings;
ALTER TABLE mqtt_settings_strict RENAME TO mqtt_settings;
