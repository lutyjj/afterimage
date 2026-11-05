$version: "2"

namespace afterimage.device

use aws.protocols#restJson1
use smithy.framework#ValidationException

/// The API that TRMNL firmware calls. Paths, headers and fields follow TRMNL's
/// published OpenAPI document; `model/trmnl/device-api.json` is the excerpt that
/// the conformance test compares against. Status values and header formats follow
/// the firmware, which is where the published document is incomplete.
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
    ]
    errors: [
        ValidationException
        InternalFailure
    ]
}

/// Registers a device by its MAC address and issues its access key.
///
/// The HTTP status is always 200 and `status` carries the outcome: 200 when the
/// device is registered, 404 when it is not, after which the firmware shows its
/// friendly ID and sleeps before it asks again.
@suppress(["HttpMethodSemantics"])
@http(method: "GET", uri: "/api/setup", code: 200)
operation SetupDevice {
    input := {
        /// The device's MAC address, such as `41:B4:10:39:A1:24`.
        @required
        @httpHeader("ID")
        id: String

        /// The `DEVICE_MODEL` the firmware was built for.
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
/// acts on three values: 0 shows `image_url`, 202 shows the device's friendly ID
/// because no account has claimed it, and 500 makes the device forget its key
/// and run setup again.
@suppress(["HttpMethodSemantics"])
@http(method: "GET", uri: "/api/display", code: 200)
operation GetDisplay {
    input := {
        @required
        @httpHeader("Access-Token")
        accessToken: String

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

        @httpHeader("Special-Function")
        specialFunction: String

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

/// The screen the device shows now, with the same `status` values as GetDisplay.
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
