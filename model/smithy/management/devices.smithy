$version: "2"

namespace afterimage.management

/// The six-character code a device shows until it is claimed.
@pattern("^[2-9A-HJ-NP-Z]{6}$")
string ClaimCode

@length(min: 1, max: 64)
string DeviceName

/// A device's MAC address, as its firmware sends it: six upper-case hexadecimal
/// octets joined by colons.
@pattern("^[0-9A-F]{2}(:[0-9A-F]{2}){5}$")
string MacAddress

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

    /// The colors the display shows, in TRMNL's order, with grays from black to
    /// white; absent for a palette of thousands of colors.
    colors: Colors
}

list Colors {
    member: Color
}

/// `#rrggbb`.
@pattern("^#[0-9a-f]{6}$")
string Color

list Palettes {
    member: Palette
}

/// The latest value of each reading the device reported; a reading it never sent
/// is absent.
structure Readings {
    /// Volts.
    batteryVoltage: Double

    /// 0 to 100: what the device reports, or else TRMNL's estimate from the
    /// voltage, since only TRMNL X reports a charge.
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

    @required
    mac: MacAddress

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

    /// What the device shows while it is awake.
    @required
    shows: Shows

    /// When the device sleeps, and what it shows meanwhile.
    @required
    sleep: Sleep

    /// Whether the device is held asleep outside its sleep windows until let go.
    /// Held, it shows its sleep screen and checks in hourly to learn of the
    /// release, since it cannot be woken from the server.
    @required
    sleepHeld: Boolean

    /// The last check-in; absent until the claimed device checks in.
    seenAt: Instant

    /// When the server asked the device to check in next; absent until the
    /// claimed device checks in. A device quiet long past it may be off or out
    /// of range.
    wakesAt: Instant

    /// When the device counts as not seen lately should it not check in before:
    /// its refresh interval and ten minutes past its wake. Absent until it checks
    /// in. A device quiet past it may be off, out of range or out of battery.
    overdueAt: Instant

    /// The screen the device shows now; absent before its first image.
    showing: Showing

    @required
    firmware: DeviceFirmware
}

list Devices {
    member: Device
}

structure Showing {
    /// Absent when the device shows a screen of the server's own, such as the one
    /// for an empty playlist or the one that says when it wakes.
    screenId: Id

    @required
    renderedAt: Instant

    /// Whether the image is the device's sleep screen, which it shows until
    /// `wakesAt`.
    @required
    asleep: Boolean
}

@range(min: 1, max: 1440)
integer RefreshMinutes

/// The screens a device shows, in order. At each check-in the device shows the
/// next entry whose days and hours include now, and wakes again after its refresh
/// interval, when the set of current entries changes, or when it falls asleep,
/// whichever comes first.
@length(max: 32)
list Playlist {
    member: PlaylistEntry
}

structure PlaylistEntry with [Hours] {
    @required
    screenId: Id
}

/// When a playlist entry or a sleep window is current, in the server's time
/// zone.
@mixin
structure Hours {
    /// Days it is current; absent means every day.
    days: Weekdays

    /// When it becomes current each day, `HH:MM`; absent means midnight.
    from: TimeOfDay

    /// When it stops being current, exclusive; absent means midnight at the end
    /// of the day. An `until` before `from` runs past midnight and belongs to the
    /// day it starts; an `until` equal to `from` makes it current all day.
    until: TimeOfDay
}

/// What an awake device shows. A pinned screen stays until set back to the
/// playlist, which waits where it was; deleting the screen sets it back.
union Shows {
    /// Its playlist.
    playlist: Unit

    /// One screen, at every check-in.
    screenId: Id
}

/// When a device sleeps. While a window is current the device shows its sleep
/// screen and does not wake until no window is, or for a day at most; its
/// playlist waits where it was. Awake, it wakes again at the next window's
/// start at the latest.
structure Sleep {
    /// Empty for a device that never sleeps.
    @required
    windows: SleepWindows

    /// The screen shown while the device sleeps; absent for the server's own,
    /// which says when it wakes. A sleep screen that cannot be drawn gives way to
    /// the server's own.
    screenId: Id
}

@length(max: 14)
list SleepWindows {
    member: SleepWindow
}

structure SleepWindow with [Hours] {}

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
@optionalAuth
@tags(["mcp"])
operation ListDisplayModels {
    output := {
        @required
        models: DisplayModels
    }
}

/// The account's devices, with their playlists, readings and the screen each
/// shows now.
@readonly
@http(method: "GET", uri: "/manage/devices")
@optionalAuth
@tags(["mcp"])
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
@optionalAuth
@tags(["mcp"])
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

/// One device, with its playlist, readings and the screen it shows now.
@readonly
@http(method: "GET", uri: "/manage/devices/{deviceId}")
@optionalAuth
@tags(["mcp"])
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
@optionalAuth
@tags(["mcp"])
operation UpdateDevice {
    input := {
        @required
        @httpLabel
        deviceId: Id

        name: DeviceName

        display: Display

        refreshMinutes: RefreshMinutes

        /// The board whose firmware the device installs, from
        /// `ListFirmwareBoards`. Choosing a board withdraws an install request the
        /// device has not received.
        firmwareBoard: FirmwareBoardName

        /// What the device shows while awake, from its next check-in.
        shows: Shows

        /// Holds the device asleep outside its sleep windows, or lets it go. A
        /// sleeping device learns of a release at its next hourly check-in.
        sleepHeld: Boolean
    }

    output := {
        @required
        device: Device
    }

    errors: [
        NotFound
        UpstreamFailure
    ]
}

/// Replaces the device's playlist.
@idempotent
@http(method: "PUT", uri: "/manage/devices/{deviceId}/playlist")
@optionalAuth
@tags(["mcp"])
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

/// Replaces when the device sleeps and what it shows meanwhile. A device takes it
/// at its next check-in: a sleeping device makes that when its sleep ends, or
/// when someone presses its button.
@idempotent
@http(method: "PUT", uri: "/manage/devices/{deviceId}/sleep")
@optionalAuth
@tags(["mcp"])
operation SetSleep {
    input := {
        @required
        @httpLabel
        deviceId: Id

        @required
        sleep: Sleep
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
@optionalAuth
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

/// The image the device shows now.
@readonly
@http(method: "GET", uri: "/manage/devices/{deviceId}/image")
@optionalAuth
@tags(["mcp"])
operation GetDeviceImage {
    input := {
        @required
        @httpLabel
        deviceId: Id
    }

    output := {
        @required
        @httpHeader("Content-Type")
        contentType: String

        @required
        @httpPayload
        image: Blob
    }

    errors: [
        NotFound
    ]
}
