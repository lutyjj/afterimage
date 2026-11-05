$version: "2"

namespace afterimage.management

use aws.protocols#restJson1
use smithy.framework#ValidationException

/// The API the web UI uses: sign-in, devices, screens and their playlists.
///
/// Every operation needs a session except those that exist to start one
/// (`GetSignInOptions`, `CreateFirstAccount`, `SignIn`, `StartOidcSignIn`,
/// `CompleteOidcSignIn`). Every resource belongs to the signed-in account; another
/// account's resource answers `NotFound`, as a missing one does.
@restJson1
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
        ListScreens
        CreateScreen
        GetScreen
        UpdateScreen
        DeleteScreen
        ListSecretNames
    ]
    errors: [
        ValidationException
        Unauthenticated
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
