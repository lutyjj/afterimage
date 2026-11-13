-- Times are seconds since the Unix epoch.

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
    -- The first account administers the server; an account made later does not.
    administrator INTEGER NOT NULL DEFAULT 0 CHECK (administrator IN (0, 1)),
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

-- A personal API token: what a script or an agent signs in with. Like a session,
-- the holder keeps an opaque random secret and the database only its SHA-256;
-- unlike one, it has a name and lasts until it is revoked.
CREATE TABLE access_tokens (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_hash BLOB NOT NULL UNIQUE,
    -- 1 when the token may only read: the operations the model marks @readonly.
    read_only INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    -- The last request that used it; NULL until one did.
    last_used_at INTEGER
) STRICT;

CREATE INDEX access_tokens_by_account ON access_tokens (account_id);

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
    created_at INTEGER NOT NULL,
    -- The board whose firmware the device installs, as a user chose it: a
    -- `keyname` from TRMNL's firmware list. NULL leaves the choice to the model
    -- the firmware reports.
    firmware_board TEXT,
    -- The playlist the device shows while no slot of its schedule is current;
    -- NULL shows that nothing is scheduled.
    playlist_id INTEGER REFERENCES playlists (id) ON DELETE SET NULL,
    -- What the device shows while awake instead of its schedule, until set back:
    -- a playlist, or else a screen; both NULL follow the schedule.
    pinned_playlist_id INTEGER REFERENCES playlists (id) ON DELETE SET NULL,
    pinned_screen_id INTEGER REFERENCES screens (id) ON DELETE SET NULL,
    -- The position shown last in `position_playlist_id`; the next check-in moves
    -- past it, and another playlist starts at its first.
    playlist_position INTEGER,
    position_playlist_id INTEGER REFERENCES playlists (id) ON DELETE SET NULL,
    -- The screen the device shows while it sleeps; NULL for the server's own.
    sleep_screen_id INTEGER REFERENCES screens (id) ON DELETE SET NULL,
    -- Whether the device is held asleep outside its sleep windows.
    sleep_held INTEGER NOT NULL DEFAULT 0 CHECK (sleep_held IN (0, 1)),
    -- The image the device shows: its file name, the screen it was drawn from
    -- (NULL for one of the server's own), the playlist (NULL for a pinned screen,
    -- a sleep screen, or a check-in without a playlist), whether it is the sleep
    -- screen, and when it was drawn.
    image_name TEXT,
    image_screen_id INTEGER REFERENCES screens (id) ON DELETE SET NULL,
    image_playlist_id INTEGER REFERENCES playlists (id) ON DELETE SET NULL,
    image_asleep INTEGER NOT NULL DEFAULT 0 CHECK (image_asleep IN (0, 1)),
    rendered_at INTEGER,
    -- When the server asked the device to check in next.
    wakes_at INTEGER
) STRICT;

-- A screen document, stored as JSON in the shape of the model's ScreenDocument.
CREATE TABLE screens (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    document TEXT NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

-- An account's playlist: screens a device shows in turn. Any number of devices
-- show one playlist, through their schedules or pinned.
CREATE TABLE playlists (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    name TEXT NOT NULL
) STRICT;

-- A playlist's screens, in `position` order; a screen may stand in it more than once.
CREATE TABLE playlist_screens (
    playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    screen_id INTEGER NOT NULL REFERENCES screens (id) ON DELETE CASCADE,
    PRIMARY KEY (playlist_id, position)
) STRICT;

-- When a device shows another playlist than its usual one, in `position` order.
-- No two of a device's slots are current at once.
CREATE TABLE schedule_slots (
    device_id INTEGER NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    -- Weekdays as bits, Monday as 1 through Sunday as 64; NULL is every day.
    days INTEGER,
    -- Minutes after midnight; NULL is midnight.
    from_minute INTEGER,
    until_minute INTEGER,
    PRIMARY KEY (device_id, position)
) STRICT;

-- When a device sleeps, in `position` order: windows of the same days and times
-- as a schedule slot's.
CREATE TABLE sleep_windows (
    device_id INTEGER NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    -- Weekdays as bits, Monday as 1 through Sunday as 64; NULL is every day.
    days INTEGER,
    -- Minutes after midnight; NULL is midnight.
    from_minute INTEGER,
    until_minute INTEGER,
    PRIMARY KEY (device_id, position)
) STRICT;

-- A device's latest firmware install request, kept until another replaces it.
CREATE TABLE firmware_updates (
    device_id INTEGER PRIMARY KEY REFERENCES devices (id) ON DELETE CASCADE,
    board TEXT NOT NULL,
    -- As the firmware reports it in FW-Version, such as 1.8.18.
    version TEXT NOT NULL,
    -- The app image's file name in the firmware cache: its SHA-256 and `.bin`.
    image TEXT NOT NULL,
    requested_at INTEGER NOT NULL,
    -- When a check-in's answer carried the request; NULL until one did.
    sent_at INTEGER,
    -- When the device checked in again after that, and whether it reported
    -- `version` then.
    finished_at INTEGER,
    installed INTEGER,
    CHECK ((finished_at IS NULL) = (installed IS NULL)),
    CHECK (finished_at IS NULL OR sent_at IS NOT NULL)
) STRICT;

-- The MQTT broker the administrator set, when the deployment sets none: at most
-- one row. The password is kept as written, since the server sends it to the
-- broker; it never leaves in an answer.
CREATE TABLE mqtt_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    url TEXT NOT NULL,
    username TEXT,
    password TEXT,
    topic TEXT NOT NULL,
    discovery_prefix TEXT NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
