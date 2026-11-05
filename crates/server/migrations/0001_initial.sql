-- Times are seconds since the Unix epoch. Until the first release this file holds the
-- whole schema and changes in place (AGENTS.md).

-- A person who signs in, with a password, through the OIDC provider, or both.
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    username TEXT UNIQUE,
    -- An argon2id hash in PHC string format; set exactly when `username` is.
    password_hash TEXT,
    oidc_issuer TEXT,
    oidc_subject TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE (oidc_issuer, oidc_subject),
    CHECK ((username IS NULL) = (password_hash IS NULL)),
    CHECK ((oidc_issuer IS NULL) = (oidc_subject IS NULL))
) STRICT;

-- A browser's sign-in. The cookie carries the token; only its SHA-256 is stored.
CREATE TABLE sessions (
    token_hash BLOB PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL
) STRICT;

-- An OIDC sign-in between the redirect to the provider and its answer, bound to
-- the browser by a cookie whose SHA-256 is `binding_hash`.
CREATE TABLE oidc_flows (
    binding_hash BLOB PRIMARY KEY,
    state TEXT NOT NULL,
    nonce TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    expires_at INTEGER NOT NULL
) STRICT;

-- One setup of a device: the key it holds and the code it shows. A device that runs
-- setup again gets another registration, and the older one keeps its key.
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
    -- When an unclaimed registration ends unless the device checks in before then;
    -- NULL once a device holds it, because a claimed registration does not end.
    expires_at INTEGER,
    -- 1 when the device's next check-in tells it to erase its settings, after which
    -- the registration is deleted.
    reset INTEGER NOT NULL DEFAULT 0,
    -- The panel size the device last reported.
    width INTEGER,
    height INTEGER,
    -- The latest readings; a check-in that omits one keeps the last known value.
    battery_voltage REAL,
    battery_percent REAL,
    signal_dbm INTEGER,
    firmware_version TEXT
) STRICT;

-- A claimed device: an account's name and settings for the registration it holds.
CREATE TABLE devices (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    registration_id INTEGER NOT NULL UNIQUE REFERENCES registrations (id),
    name TEXT NOT NULL,
    -- A model name and palette id from TRMNL's catalogue.
    display_model TEXT NOT NULL,
    palette TEXT NOT NULL,
    refresh_minutes INTEGER NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;

-- A screen document, stored as JSON in the shape of the model's ScreenDocument.
CREATE TABLE screens (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    document TEXT NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

-- A device's playlist, in `position` order.
CREATE TABLE playlist_entries (
    device_id INTEGER NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    screen_id INTEGER NOT NULL REFERENCES screens (id) ON DELETE CASCADE,
    -- Weekdays as bits, Monday as 1 through Sunday as 64; NULL is every day.
    days INTEGER,
    -- Minutes after midnight; NULL is midnight.
    from_minute INTEGER,
    until_minute INTEGER,
    PRIMARY KEY (device_id, position)
) STRICT;
