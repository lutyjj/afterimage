$version: "2"

namespace afterimage.management

/// The six-character code a device shows until it is claimed.
@pattern("^[2-9A-HJ-NP-Z]{6}$")
string ClaimCode

@length(min: 1, max: 64)
string DeviceName

/// A model name from TRMNL's model list, such as `og_plus`.
@length(min: 1, max: 64)
string DisplayModelName

/// A palette id from TRMNL's palette list, such as `gray-4`.
@length(min: 1, max: 64)
string PaletteId

/// How the server renders for a device: one of TRMNL's display models and one of
/// the palettes that model supports.
structure Display {
    @required
    model: DisplayModelName

    @required
    palette: PaletteId
}

/// A display model as TRMNL publishes it, with the palettes it supports.
structure DisplayModel {
    @required
    name: DisplayModelName

    @required
    label: String

    @required
    width: Integer

    @required
    height: Integer

    @required
    palettes: Palettes
}

list DisplayModels {
    member: DisplayModel
}

structure Palette {
    @required
    id: PaletteId

    @required
    name: String
}

list Palettes {
    member: Palette
}

/// The latest value of each reading the device reported; a reading it never sent
/// is absent.
structure Readings {
    /// Volts.
    batteryVoltage: Double

    /// 0 to 100.
    batteryPercent: Double

    /// dBm.
    signal: Integer

    firmwareVersion: String
}

structure Device {
    @required
    id: Id

    @required
    name: DeviceName

    /// The device's MAC address.
    @required
    mac: String

    /// The model name the device's firmware reports.
    @required
    firmwareModel: String

    @required
    display: Display

    /// Minutes between check-ins when no scheduled change comes sooner.
    @required
    refreshMinutes: RefreshMinutes

    @required
    playlist: Playlist

    @required
    readings: Readings

    /// The last check-in; absent until the claimed device checks in.
    seenAt: Instant

    /// The screen the device shows now; absent before its first image.
    showing: Showing
}

list Devices {
    member: Device
}

structure Showing {
    /// Absent when the device shows a screen of the server's own, such as the one
    /// for an empty playlist.
    screenId: Id

    @required
    renderedAt: Instant
}

@range(min: 1, max: 1440)
integer RefreshMinutes

/// The screens a device shows, in order. At each check-in the device shows the
/// next entry whose days and hours include now, and wakes again after its refresh
/// interval or when the set of current entries changes, whichever comes first.
@length(max: 32)
list Playlist {
    member: PlaylistEntry
}

structure PlaylistEntry {
    @required
    screenId: Id

    /// Days the entry is current; absent means every day.
    days: Weekdays

    /// When the entry becomes current each day, `HH:MM` in the server's time zone;
    /// absent means midnight.
    from: TimeOfDay

    /// When it stops being current, exclusive; absent means midnight at the end of
    /// the day. An `until` before `from` runs past midnight; an `until` equal to
    /// `from` makes the entry current all day.
    until: TimeOfDay
}

@uniqueItems
@length(min: 1, max: 7)
list Weekdays {
    member: Weekday
}

enum Weekday {
    MONDAY = "monday"
    TUESDAY = "tuesday"
    WEDNESDAY = "wednesday"
    THURSDAY = "thursday"
    FRIDAY = "friday"
    SATURDAY = "saturday"
    SUNDAY = "sunday"
}

@pattern("^([01][0-9]|2[0-3]):[0-5][0-9]$")
string TimeOfDay

/// TRMNL's display models and their palettes, for choosing a device's display.
@readonly
@http(method: "GET", uri: "/manage/display-models")
operation ListDisplayModels {
    output := {
        @required
        models: DisplayModels
    }
}

@readonly
@http(method: "GET", uri: "/manage/devices")
operation ListDevices {
    output := {
        @required
        devices: Devices
    }
}

/// Claims the device showing `code`. When the account already has a device with
/// the same MAC, that device takes the new registration and keeps its name,
/// settings and playlist; its old registration ends.
@http(method: "POST", uri: "/manage/devices", code: 201)
operation ClaimDevice {
    input := {
        @required
        code: ClaimCode

        /// Absent: the device keeps its name, or a new device gets one from its
        /// code.
        name: DeviceName
    }

    output := {
        @required
        device: Device

        /// The claim replaced the registration of a device the account had.
        @required
        replaced: Boolean
    }

    errors: [
        NotFound
    ]
}

@readonly
@http(method: "GET", uri: "/manage/devices/{deviceId}")
operation GetDevice {
    input := {
        @required
        @httpLabel
        deviceId: Id
    }

    output := {
        @required
        device: Device
    }

    errors: [
        NotFound
    ]
}

/// Changes the given settings and leaves the others as they are.
@http(method: "PATCH", uri: "/manage/devices/{deviceId}")
operation UpdateDevice {
    input := {
        @required
        @httpLabel
        deviceId: Id

        name: DeviceName

        display: Display

        refreshMinutes: RefreshMinutes
    }

    output := {
        @required
        device: Device
    }

    errors: [
        NotFound
    ]
}

/// Replaces the device's playlist.
@idempotent
@http(method: "PUT", uri: "/manage/devices/{deviceId}/playlist")
operation SetPlaylist {
    input := {
        @required
        @httpLabel
        deviceId: Id

        @required
        playlist: Playlist
    }

    output := {
        @required
        device: Device
    }

    errors: [
        NotFound
    ]
}

/// Removes the device from the account. Without `reset` the device goes back to
/// showing its claim code, so it can be claimed again; with `reset` its next
/// check-in erases its Wi-Fi networks and settings, and it starts over at its setup
/// portal.
@idempotent
@http(method: "DELETE", uri: "/manage/devices/{deviceId}", code: 204)
operation DeleteDevice {
    input := {
        @required
        @httpLabel
        deviceId: Id

        @httpQuery("reset")
        reset: Boolean
    }

    errors: [
        NotFound
    ]
}
