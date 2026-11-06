$version: "2"

namespace afterimage.management

/// The MQTT broker the server publishes its devices to, with Home Assistant's
/// discovery. The deployment sets it through `AFTERIMAGE_MQTT_URL` and the settings
/// beside it; where it does, these settings only show what it set. Where it does
/// not, the administrator sets the broker here, and the server connects to it at
/// once.
@readonly
@http(method: "GET", uri: "/manage/server/mqtt")
@optionalAuth
operation GetMqttSettings {
    output := {
        @required
        settings: MqttSettings
    }

    errors: [
        Forbidden
    ]
}

/// Sets the broker, or with no `url` stops publishing. The server clears what it
/// published under the settings it had, then connects with these.
@idempotent
@http(method: "PUT", uri: "/manage/server/mqtt")
@optionalAuth
operation UpdateMqttSettings {
    input := {
        /// `mqtt://` or `mqtts://`, the host and its port, without credentials;
        /// absent stops publishing.
        url: MqttUrl

        username: MqttUsername

        /// Absent keeps the password the server has; empty removes it.
        password: MqttPassword

        /// The topic every device is published under, also the server's client ID
        /// at the broker and the start of each device's discovery ID.
        topic: MqttTopic = "afterimage"

        /// Home Assistant's discovery prefix.
        discoveryPrefix: MqttDiscoveryPrefix = "homeassistant"
    }

    output := {
        @required
        settings: MqttSettings
    }

    errors: [
        Forbidden
        Conflict
    ]
}

structure MqttSettings {
    /// Whether the deployment sets the broker, so these settings cannot change.
    @required
    setByDeployment: Boolean

    /// Absent when the server publishes to no broker.
    url: MqttUrl

    username: MqttUsername

    /// Whether a password is set. The password itself is never shown.
    @required
    passwordSet: Boolean

    @required
    topic: MqttTopic

    @required
    discoveryPrefix: MqttDiscoveryPrefix

    @required
    connection: MqttConnection
}

enum MqttConnection {
    /// No broker is set.
    OFF = "off"

    /// The server is connecting, or the broker cannot be reached and it is trying
    /// again.
    CONNECTING = "connecting"

    CONNECTED = "connected"
}

@pattern("^mqtts?://")
@length(min: 8, max: 512)
string MqttUrl

@length(min: 1, max: 256)
string MqttUsername

@length(max: 256)
string MqttPassword

/// One topic level of letters, digits, `_` and `-`, as Home Assistant's discovery
/// IDs take.
@pattern("^[A-Za-z0-9_-]+$")
@length(min: 1, max: 64)
string MqttTopic

/// Topic levels without wildcards or empty levels.
@pattern("^[!\"$-*,-.0-~]+(/[!\"$-*,-.0-~]+)*$")
@length(min: 1, max: 128)
string MqttDiscoveryPrefix
