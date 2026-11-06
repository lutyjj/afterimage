$version: "2"

// A management operation states how it authenticates, and an operation a token
// may call reads with GET exactly when it is @readonly: the server lets a
// read-only token make GET requests only. DANGER fails the build unless an
// operation suppresses these ids by name, which HttpMethodSemantics does not.
metadata validators = [
    {
        name: "EmitEachSelector"
        id: "CredentialDeclared"
        message: "A management operation needs @auth([]) when it starts a session, or @optionalAuth when it takes a session or a token."
        severity: "DANGER"
        configuration: {
            selector: "service [id = afterimage.management#Management] ~> operation :not([trait|auth]) :not([trait|optionalAuth])"
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
]

namespace afterimage.management

use aws.protocols#restJson1
use smithy.framework#ValidationException

/// The API the web UI uses: sign-in, devices, their firmware, screens and their
/// playlists, and the personal API tokens scripts and agents use instead.
///
/// Every operation needs a credential except those that exist to start a
/// session (`GetSignInOptions`, `CreateFirstAccount`, `SignIn`,
/// `StartOidcSignIn`, `CompleteOidcSignIn`): the session cookie a browser holds,
/// or a personal API token as `Authorization: Bearer <secret>`, which wins when
/// a request carries both. Every resource belongs to the signed-in account;
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
        SetPlaylist
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
        GetServer
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
operation GetServer {
    output := {
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
