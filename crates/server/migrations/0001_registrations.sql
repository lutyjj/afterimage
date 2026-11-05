-- One setup of a device: the key it holds and the code it shows. A device that runs
-- setup again gets another registration, and the older one keeps its key. Times are
-- seconds since the Unix epoch.
CREATE TABLE registrations (
    id INTEGER PRIMARY KEY,
    mac TEXT NOT NULL,
    -- The code the device shows so that a user can claim it.
    friendly_id TEXT NOT NULL UNIQUE,
    -- SHA-256 of the access key; the key itself is never stored.
    key_hash BLOB NOT NULL UNIQUE,
    -- The firmware's DEVICE_MODEL.
    model TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    -- The last check-in; NULL until the device uses its key.
    seen_at INTEGER,
    -- The latest readings; a check-in that omits one keeps the last known value.
    battery_voltage REAL,
    battery_percent REAL,
    signal_dbm INTEGER,
    firmware_version TEXT
) STRICT;
