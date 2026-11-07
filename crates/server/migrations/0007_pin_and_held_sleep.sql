-- The screen a device shows while awake instead of its playlist, until set
-- back; NULL follows the playlist.
ALTER TABLE devices ADD COLUMN pinned_screen_id INTEGER REFERENCES screens (id) ON DELETE SET NULL;
-- Whether the device is held asleep outside its sleep windows.
ALTER TABLE devices ADD COLUMN sleep_held INTEGER NOT NULL DEFAULT 0 CHECK (sleep_held IN (0, 1));
