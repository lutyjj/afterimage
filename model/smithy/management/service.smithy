$version: "2"

// A management operation states how it authenticates, and an operation a token
// may call reads with GET exactly when it is @readonly: the server lets a
// read-only token make GET requests only. DANGER fails the build unless an
// operation suppresses these ids by name, which HttpMethodSemantics does not.
metadata validators = [
    {
        name: "EmitEachSelector"
        id: "CredentialDeclared"
        message: "A management operation needs @auth([]) when it starts a session, or @optionalAuth when it takes a session or a token; the server has no other mode."
        severity: "DANGER"
        configuration: {
            selector: "service [id = afterimage.management#Management] ~> operation :not([trait|optionalAuth]) :not([trait|auth|(length) = 0])"
        }
    }
    {
        name: "EmitEachSelector"
        id: "ReadOnlyIsGet"
        message: "An operation a token may call uses GET exactly when it is @readonly, since a read-only token may make GET requests only."
        severity: "DANGER"
        configuration: {
            selector: "service [id = afterimage.management#Management] ~> operation [trait|optionalAuth] :is([trait|http|method = GET] :not([trait|readonly]), [trait|readonly] :not([trait|http|method = GET]))"
        }
    }
    {
        name: "EmitEachSelector"
        id: "ToolDescribed"
        message: "An operation tagged `mcp` is a tool of the MCP endpoint, which a client calls with a token and knows by its documentation: it needs both @optionalAuth and a doc comment."
        severity: "DANGER"
        configuration: {
            selector: "service [id = afterimage.management#Management] ~> operation [trait|tags|(values) = mcp] :is(:not([trait|documentation]), :not([trait|optionalAuth]))"
        }
    }
]

namespace afterimage.management

use aws.protocols#restJson1
use smithy.framework#ValidationException

/// The API the web UI uses: sign-in, devices and their firmware, screens,
/// playlists, and the personal access tokens scripts and agents use instead.
///
/// A screen is a document of components that templates fill from data sources.
/// `PreviewScreen` draws a document on a display and answers the image, what each
/// source returned and every problem, so a document is checked before
/// `CreateScreen` or `UpdateScreen` saves it. A playlist (`CreatePlaylist`) is
/// screens shown in turn, and any number of devices show it. A device's schedule
/// (`SetSchedule`) names the playlist it usually shows and the slots of days and
/// times, in the server's time zone (`GetServer`, which also names the secrets
/// source requests may read), when another one takes its place. It sleeps in its
/// sleep windows (`SetSleep`), which take the same days and times.
///
/// Every operation needs a credential except those that exist to start a
/// session (`GetSignInOptions`, `CreateFirstAccount`, `SignIn`,
/// `StartOidcSignIn`, `CompleteOidcSignIn`): the session cookie a browser holds,
/// or a personal access token as `Authorization: Bearer <secret>`, which wins when
/// a request carries both; an `Authorization` of another scheme leaves the
/// session to decide. Every resource belongs to the signed-in account;
/// another account's resource answers `NotFound`, as a missing one does. A
/// read-only token may call only the operations marked `@readonly`, and no token
/// may create or revoke a token or sign out; each answers `Forbidden`.
@restJson1
@httpBearerAuth
@title("Afterimage management API")
service Management {
    version: "1"
    operations: [
        GetSignInOptions
        CreateFirstAccount
        SignIn
        StartOidcSignIn
        CompleteOidcSignIn
        SignOut
        GetAccount
        ListDisplayModels
        ListDevices
        ClaimDevice
        GetDevice
        UpdateDevice
        SetSchedule
        SetSleep
        DeleteDevice
        GetDeviceImage
        ListFirmwareBoards
        InstallFirmware
        CancelFirmwareUpdate
        ListScreens
        CreateScreen
        GetScreen
        UpdateScreen
        DeleteScreen
        PreviewScreen
        ListPlaylists
        CreatePlaylist
        GetPlaylist
        UpdatePlaylist
        DeletePlaylist
        GetServer
        GetMqttSettings
        UpdateMqttSettings
        CreateAccessToken
        ListAccessTokens
        RevokeAccessToken
    ]
    errors: [
        ValidationException
        Unauthenticated
        Forbidden
        InternalFailure
    ]
}

/// The request carries no session, or one that has ended.
@error("client")
@httpError(401)
structure Unauthenticated {
    @required
    message: String
}

/// The resource does not exist, or belongs to another account.
@error("client")
@httpError(404)
structure NotFound {
    @required
    message: String
}

/// The request conflicts with the current state, which `message` describes.
@error("client")
@httpError(409)
structure Conflict {
    @required
    message: String
}

/// The credential may not do this: a token creating or revoking tokens or
/// signing out, a read-only token changing anything, or an account that does not
/// administer the server reading or changing its settings.
@error("client")
@httpError(403)
structure Forbidden {
    @required
    message: String
}

/// The server could not complete the request.
@error("server")
@httpError(500)
structure InternalFailure {
    @required
    message: String
}

/// A row identifier the server assigns.
@range(min: 1)
long Id

/// Seconds since the Unix epoch.
@timestampFormat("epoch-seconds")
timestamp Instant

list Names {
    member: String
}

/// What the deployment gives every account: the time zone of playlists and dates,
/// and the secrets data source requests may read.
@readonly
@http(method: "GET", uri: "/manage/server")
@optionalAuth
@tags(["mcp"])
operation GetServer {
    output := {
        /// The server's release, such as `0.1.0`.
        @required
        version: String

        /// The IANA name of the time zone that playlist times and dates on screens
        /// follow, such as `Europe/Berlin`.
        @required
        timeZone: String

        /// The names of the secrets the deployment provides, which data source
        /// requests read as `secrets.NAME`. The values never appear in an answer,
        /// but every account's requests may send them anywhere: a deployment gives
        /// its secrets to every account it lets sign in.
        @required
        secretNames: Names

        /// The display a device of unknown model and size starts with, from TRMNL's
        /// catalogue; a client draws on it where no device names one, such as a
        /// preview before any device is claimed.
        @required
        defaultDisplay: Display
    }
}
