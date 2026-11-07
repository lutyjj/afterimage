-- When a device sleeps, in `position` order: windows of the same days and times
-- as a playlist entry's.
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

-- The screen a device shows while it sleeps; NULL for the server's own.
ALTER TABLE devices ADD COLUMN sleep_screen_id INTEGER REFERENCES screens (id) ON DELETE SET NULL;
-- When the server asked the device to check in next.
ALTER TABLE devices ADD COLUMN wakes_at INTEGER;
-- Whether the image the device shows is its sleep screen.
ALTER TABLE devices ADD COLUMN image_asleep INTEGER NOT NULL DEFAULT 0 CHECK (image_asleep IN (0, 1));
