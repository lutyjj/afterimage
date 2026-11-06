-- The board whose firmware a device installs, as a user chose it: a `keyname` from
-- TRMNL's firmware list. NULL leaves the choice to the model the firmware reports.
ALTER TABLE devices ADD COLUMN firmware_board TEXT;

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
