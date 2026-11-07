$version: "2"

namespace afterimage.device

use aws.protocols#restJson1
use smithy.framework#ValidationException

/// The device API TRMNL documents. Paths, headers and fields follow TRMNL's
/// published OpenAPI document; `model/trmnl/device-api.json` is the excerpt that
/// the conformance test compares against. Status values and header formats follow
/// the firmware (release 1.8.18), which is where the published document is
/// incomplete.
///
/// Headers are text on the wire and the firmware's values do not always match the
/// published types (`Battery-Capacity` is `3000/3500`, `Battery-Charging` is `1`),
/// so every header is a string here and the server parses what it uses.
@restJson1
@title("TRMNL device API")
service DeviceApi {
    operations: [
        SetupDevice
        GetDisplay
        GetCurrentScreen
        CreateDeviceLog
        GetScreenImage
        GetFirmwareImage
    ]
    errors: [
        ValidationException
        InternalFailure
    ]
}

/// Registers a device by its MAC address and issues its access key.
///
/// The HTTP status is always 200 and `status` carries the outcome. On 200 the
/// firmware stores the key and the friendly ID and shows the ID with `message`. On
/// 404 it shows the image at `image_url`, or else `message`, then sleeps for its
/// default interval and asks again.
@suppress(["HttpMethodSemantics"])
@http(method: "GET", uri: "/api/setup", code: 200)
operation SetupDevice {
    input := {
        /// The device's MAC address, such as `41:B4:10:39:A1:24`.
        @required
        @httpHeader("ID")
        id: String

        /// The `DEVICE_MODEL` the firmware was built for. Firmware before 1.8 sends it
        /// empty here (`performApiSetup` in `src/bl.cpp` leaves it unset); its
        /// check-ins name it.
        @required
        @httpHeader("Model")
        model: String

        /// The ePaper panel fitted to the device.
        @httpHeader("Panel-Rev")
        panelRev: String
    }

    output := {
        @required
        status: Integer

        /// The device's access key, sent back in `Access-Token`.
        @jsonName("api_key")
        apiKey: String

        /// The short code the device shows so that a user can claim it.
        @jsonName("friendly_id")
        friendlyId: String

        @jsonName("image_url")
        imageUrl: String

        message: String
    }
}

/// The device's check-in: it reports its state and learns what to show next.
///
/// The HTTP status is always 200 and `status` carries the outcome. The firmware
/// acts on three values: 0 shows `image_url`; 202 shows the device's friendly ID
/// and polls on the firmware's own schedule, every 5 seconds at first and backing
/// off to hourly; 500, like `reset_firmware`, erases the device's saved
/// Wi-Fi networks and every stored setting, the server URL included, and restarts
/// it into its setup portal, where a person must provision it again.
@suppress(["HttpMethodSemantics"])
@http(method: "GET", uri: "/api/display", code: 200)
operation GetDisplay {
    input := {
        @required
        @httpHeader("Access-Token")
        accessToken: String

        /// The device's MAC address. TRMNL's document does not list it here, but the
        /// firmware sends it on every check-in (`buildDisplayHeaders`).
        @httpHeader("ID")
        id: String

        /// The `DEVICE_MODEL` the firmware was built for. TRMNL's document does not
        /// list it here, but the firmware sends it on every check-in
        /// (`buildDisplayHeaders`), where firmware before 1.8 names it for the first
        /// time.
        @httpHeader("Model")
        model: String

        /// Volts, such as `4.12`.
        @httpHeader("Battery-Voltage")
        batteryVoltage: String

        /// Percent, such as `69.4`.
        @httpHeader("Percent-Charged")
        percentCharged: String

        @httpHeader("Battery-Count")
        batteryCount: String

        /// `1` while charging, `0` otherwise.
        @httpHeader("Battery-Charging")
        batteryCharging: String

        @httpHeader("Battery-Health")
        batteryHealth: String

        @httpHeader("Battery-Current")
        batteryCurrent: String

        @httpHeader("Battery-Temp")
        batteryTemp: String

        /// Current and full capacity, such as `3000/3500`.
        @httpHeader("Battery-Capacity")
        batteryCapacity: String

        /// `true` or `false`.
        @httpHeader("USB-Connected")
        usbConnected: String

        /// `2.4` or `5`.
        @httpHeader("WiFi-Band")
        wifiBand: String

        @httpHeader("Panel-Rev")
        panelRev: String

        /// Such as `1.8.18`.
        @httpHeader("FW-Version")
        firmwareVersion: String

        @httpHeader("FW-Commit")
        firmwareCommit: String

        /// dBm, such as `-61`.
        @httpHeader("RSSI")
        rssi: String

        @httpHeader("Height")
        height: String

        @httpHeader("Width")
        width: String

        /// Why the device woke: `timer`, `button`, or `EXT0` and `EXT1` for a
        /// press on boards that wake on an external pin; `powercycle` after power
        /// on. TRMNL's document does not list it, but firmware 1.8.18 sends it on
        /// every check-in (`buildDisplayHeaders`, `parseWakeupReasonToStr`).
        @httpHeader("Update-Source")
        updateSource: String

        /// TRMNL's document names this header; firmware 1.8.18 sends
        /// `special_function` instead (below).
        @httpHeader("Special-Function")
        specialFunction: String

        /// `true` for the function configured on the server, or the name of one.
        /// The firmware's spelling of `Special-Function` (`buildDisplayHeaders`).
        @httpHeader("special_function")
        firmwareSpecialFunction: String

        @httpHeader("BASE64")
        base64: String

        @httpHeader("Sensors")
        sensors: String

        @httpHeader("Image-Cached")
        imageCached: String

        /// Seconds the device was awake during its last cycle.
        @httpHeader("Wake-Time")
        wakeTime: String
    }

    output := {
        @required
        status: Integer

        @jsonName("image_url")
        imageUrl: String

        filename: String

        /// Seconds until the next check-in. The firmware reads a missing value as 0.
        @required
        @jsonName("refresh_rate")
        refreshRate: Integer

        @required
        @jsonName("reset_firmware")
        resetFirmware: Boolean

        @required
        @jsonName("update_firmware")
        updateFirmware: Boolean

        @jsonName("firmware_url")
        firmwareUrl: String

        @jsonName("special_function")
        specialFunction: String

        action: String
    }
}

/// The screen the device shows now. TRMNL documents this operation; firmware 1.8.18
/// does not call it. The server answers with the status GetDisplay would give.
@readonly
@http(method: "GET", uri: "/api/display/current", code: 200)
operation GetCurrentScreen {
    input := {
        @required
        @httpHeader("Access-Token")
        accessToken: String
    }

    output := {
        @required
        status: Integer

        @required
        @jsonName("refresh_rate")
        refreshRate: Integer

        @jsonName("image_url")
        imageUrl: String

        filename: String

        @jsonName("rendered_at")
        @timestampFormat("date-time")
        renderedAt: Timestamp
    }
}

/// Log entries the device buffered. Each entry is an object whose fields the
/// firmware defines (`created_at`, `level`, `message`, battery and Wi-Fi state).
@http(method: "POST", uri: "/api/log", code: 204)
operation CreateDeviceLog {
    input := {
        @required
        @httpHeader("Access-Token")
        accessToken: String

        @required
        logs: DeviceLogEntries
    }

    errors: [
        UnknownDevice
    ]
}

list DeviceLogEntries {
    member: Document
}

/// The access key belongs to no device.
@error("client")
@httpError(401)
structure UnknownDevice {
    message: String
}

/// The server could not complete the request; the device tries again later.
@error("server")
@httpError(500)
structure InternalFailure {
    message: String
}

/// The image a check-in answer names in `image_url`. This operation is Afterimage's:
/// TRMNL's document leaves the image URL to the server. Firmware 1.8.18 downloads
/// it with the device's `Access-Token` (`buildImageHeaders`), and only the device
/// the image was drawn for may fetch it.
@readonly
@http(method: "GET", uri: "/api/images/{name}", code: 200)
operation GetScreenImage {
    input := {
        @required
        @httpLabel
        name: String

        @required
        @httpHeader("Access-Token")
        accessToken: String
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
        UnknownDevice
        ImageNotFound
    ]
}

/// The device has no image of that name: never drawn, or replaced by a newer one
/// a while ago.
@error("client")
@httpError(404)
structure ImageNotFound {
    message: String
}

/// The firmware a check-in answer names in `firmware_url`: the app image of a
/// release TRMNL publishes, cut from its flash image where TRMNL publishes no
/// other, which the server checked and keeps. This operation is Afterimage's: TRMNL's
/// document leaves the firmware URL to the server. The firmware downloads it
/// without headers of its own (1.6.10 `checkAndPerformFirmwareUpdate` in
/// `src/bl.cpp`, 1.8.18 `performFirmwareUpdate` in
/// `src/services/firmware_update.cpp`), and follows no redirect, so the image
/// needs no key; TRMNL publishes it anyway.
@readonly
@http(method: "GET", uri: "/api/firmware-images/{name}", code: 200)
operation GetFirmwareImage {
    input := {
        /// The image's SHA-256 in lowercase hex, and `.bin`.
        @required
        @httpLabel
        name: FirmwareImageName
    }

    output := {
        @required
        @httpHeader("Content-Type")
        contentType: String

        /// The firmware writes the image to its update partition (`Update.begin`
        /// with the response's length).
        @required
        @httpPayload
        image: Blob
    }

    errors: [
        FirmwareNotFound
    ]
}

@pattern("^[0-9a-f]{64}\\.bin$")
string FirmwareImageName

/// The server keeps no firmware image of that name.
@error("client")
@httpError(404)
structure FirmwareNotFound {
    message: String
}
