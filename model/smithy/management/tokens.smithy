$version: "2"

namespace afterimage.management

/// A token's secret: 43 characters of URL-safe base64, sent as
/// `Authorization: Bearer <secret>`. Shown once, when the token is created.
@sensitive
string TokenSecret

@length(min: 1, max: 64)
string TokenName

/// What a token may do besides acting as its owner.
enum TokenScope {
    /// Every operation the model marks `@readonly`, which are the ones read with
    /// GET.
    READ_ONLY = "read_only"
}

/// A personal API token, for scripts and agents acting for an account. It acts
/// as its owner wherever a session does, narrowed by its scope, and lasts until
/// it is revoked. The server keeps only a hash of its secret.
structure AccessToken {
    @required
    id: Id

    @required
    name: TokenName

    /// Absent: the token reaches everything its owner may, but tokens.
    scope: TokenScope

    @required
    createdAt: Instant

    /// The last request that used it; absent until one did.
    lastUsedAt: Instant
}

list AccessTokens {
    member: AccessToken
}

/// Creates a token for the signed-in account. A token cannot create one, so a
/// leaked token cannot outlive its revocation.
@http(method: "POST", uri: "/manage/account/tokens", code: 201)
@optionalAuth
operation CreateAccessToken {
    input := {
        @required
        name: TokenName

        scope: TokenScope
    }

    output := {
        @required
        token: AccessToken

        /// The only time the secret appears.
        @required
        secret: TokenSecret
    }

}

/// The account's tokens, newest first, without their secrets.
@readonly
@http(method: "GET", uri: "/manage/account/tokens")
@optionalAuth
operation ListAccessTokens {
    output := {
        @required
        tokens: AccessTokens
    }
}

/// Revokes one of the account's tokens: whatever holds it can ask nothing more.
/// A token cannot revoke one.
@idempotent
@http(method: "DELETE", uri: "/manage/account/tokens/{tokenId}", code: 204)
@optionalAuth
operation RevokeAccessToken {
    input := {
        @required
        @httpLabel
        tokenId: Id
    }

    errors: [
        NotFound
    ]
}

/// The credential may not do this: a token creating or revoking tokens, or a
/// read-only token changing anything.
@error("client")
@httpError(403)
structure Forbidden {
    @required
    message: String
}
