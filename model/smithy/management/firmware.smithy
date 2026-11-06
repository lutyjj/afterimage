$version: "2"

namespace afterimage.management

/// A board in TRMNL's firmware list (`GET /api/firmware/flash`), by its
/// `keyname`, such as `seeed_OG_DIY_Kit`.
@pattern("^[A-Za-z0-9_-]{1,64}$")
string FirmwareBoardName

/// A firmware release as the device reports it in `FW-Version`, such as `1.8.18`.
@pattern("^[0-9A-Za-z.+-]{1,32}$")
string FirmwareVersion

/// A board TRMNL builds firmware for, with its newest release.
structure FirmwareBoard {
    @required
    name: FirmwareBoardName

    /// Such as `Seeed Studio (TRMNL 7.5" OG DIY Kit)`.
    @required
    label: String

    /// Such as `ESP32-S3`.
    @required
    chipFamily: String

    /// The newest release TRMNL lists for the board, leaving out variants such as
    /// `1.7.8-sensors`; absent when it lists none.
    latestVersion: FirmwareVersion
}

list FirmwareBoards {
    member: FirmwareBoard
}

/// What the server knows about a device's firmware.
structure DeviceFirmware {
    /// The board whose firmware the device installs: the one a user chose, or else
    /// the board TRMNL names like the model the firmware reports. TRMNL publishes
    /// no other link between the two, and installing another board's firmware can
    /// leave a device unable to start until it is flashed over USB.
    board: FirmwareBoardName

    /// Whether a user chose `board`, rather than the server matching it by name.
    @required
    boardChosen: Boolean

    /// The board's newest release; absent without a board, or while TRMNL's list
    /// cannot be read.
    latestVersion: FirmwareVersion

    /// Whether `latestVersion` is newer than the release the device reports, or
    /// differs from one that does not read as a version.
    @required
    updateAvailable: Boolean

    /// The latest install request, until another replaces it.
    update: FirmwareUpdate
}

/// An install request. The next check-in's answer tells the device to download
/// the release from this server and install it; the check-in after that shows
/// whether it did.
structure FirmwareUpdate {
    @required
    version: FirmwareVersion

    @required
    state: FirmwareUpdateState

    @required
    requestedAt: Instant

    /// When a check-in's answer carried the request.
    sentAt: Instant

    /// When the device checked in again after that.
    finishedAt: Instant
}

enum FirmwareUpdateState {
    /// Waiting for the device's next check-in.
    REQUESTED = "requested"

    /// The device has the request and has not checked in since. Both firmware
    /// lines restart and check in as soon as they have tried to install it.
    SENT = "sent"

    /// The device checked in reporting `version`.
    INSTALLED = "installed"

    /// The device checked in reporting another version. Firmware 1.8 tries a
    /// failed update again only after a day, and its log says why it failed.
    NOT_INSTALLED = "not_installed"
}

/// The boards TRMNL builds firmware for, from its firmware list, for choosing a
/// device's board.
@readonly
@http(method: "GET", uri: "/manage/firmware-boards")
operation ListFirmwareBoards {
    output := {
        @required
        boards: FirmwareBoards
    }

    errors: [
        UpstreamFailure
    ]
}

/// Asks the device to install a release of its board's firmware at its next
/// check-in. The server first downloads the release's image from TRMNL, cuts the
/// app out of it and checks its checksum and SHA-256, then serves it to the
/// device itself. The request replaces any earlier one.
@http(method: "POST", uri: "/manage/devices/{deviceId}/firmware-update")
operation InstallFirmware {
    input := {
        @required
        @httpLabel
        deviceId: Id

        /// A release TRMNL lists for the device's board; absent means the newest.
        version: FirmwareVersion
    }

    output := {
        @required
        device: Device
    }

    errors: [
        NotFound
        Conflict
        UpstreamFailure
    ]
}

/// Withdraws the device's install request. A device that already received it
/// may still install the release.
@idempotent
@http(method: "DELETE", uri: "/manage/devices/{deviceId}/firmware-update")
operation CancelFirmwareUpdate {
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

/// TRMNL's firmware list or image could not be read, or the image failed its
/// checks; `message` says which.
@error("server")
@httpError(502)
structure UpstreamFailure {
    @required
    message: String
}
