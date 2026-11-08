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

-- The playlist a device shows while no slot is current; NULL shows that nothing
-- is scheduled.
ALTER TABLE devices ADD COLUMN playlist_id INTEGER REFERENCES playlists (id) ON DELETE SET NULL;
-- The playlist a device shows while awake instead of its schedule, until set
-- back; NULL follows the schedule, unless a screen is pinned.
ALTER TABLE devices ADD COLUMN pinned_playlist_id INTEGER REFERENCES playlists (id) ON DELETE SET NULL;
-- The playlist `playlist_position` counts in; another playlist starts at its first.
ALTER TABLE devices ADD COLUMN position_playlist_id INTEGER REFERENCES playlists (id) ON DELETE SET NULL;
-- The playlist the image was drawn from; NULL for a pinned screen, a sleep
-- screen, or a check-in without a playlist.
ALTER TABLE devices ADD COLUMN image_playlist_id INTEGER REFERENCES playlists (id) ON DELETE SET NULL;

-- Each device's list of screens becomes a playlist of its own, named for the
-- device, which it usually shows and goes on through from where it stood. An
-- entry's days and times have no place in a playlist and are dropped: a device
-- shows other screens at other times through the slots of its schedule. The
-- playlists table starts empty, so each new playlist takes its device's id.
INSERT INTO playlists (id, account_id, name)
SELECT id, account_id, name FROM devices
WHERE id IN (SELECT device_id FROM playlist_entries);

INSERT INTO playlist_screens (playlist_id, position, screen_id)
SELECT device_id, position, screen_id FROM playlist_entries;

UPDATE devices SET playlist_id = id, position_playlist_id = id
WHERE id IN (SELECT id FROM playlists);

DROP TABLE playlist_entries;
