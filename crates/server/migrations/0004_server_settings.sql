-- The account that administers the server, and the settings it changes there.

-- The first account administers the server; an account made later does not.
ALTER TABLE accounts ADD COLUMN administrator INTEGER NOT NULL DEFAULT 0
    CHECK (administrator IN (0, 1));
UPDATE accounts SET administrator = 1 WHERE id = (SELECT min(id) FROM accounts);

-- The MQTT broker the administrator set, when the deployment sets none: at most
-- one row. The password is kept as written, since the server sends it to the
-- broker; it never leaves in an answer.
CREATE TABLE mqtt_settings (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    url              TEXT    NOT NULL,
    username         TEXT,
    password         TEXT,
    topic            TEXT    NOT NULL,
    discovery_prefix TEXT    NOT NULL,
    updated_at       INTEGER NOT NULL
);
