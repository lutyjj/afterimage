$version: "2"

namespace afterimage.management

/// Lowercase, so one person cannot hold two spellings of one name.
@pattern("^[a-z0-9][a-z0-9._-]{1,63}$")
string Username

@sensitive
@length(min: 10, max: 256)
string Password

@length(min: 1, max: 64)
string AccountName

/// A cookie the server sets. It is httpOnly, so no script reads it: the browser
/// stores it and sends it back. A browser client never sees this member.
@sensitive
string SessionCookie

/// The cookies a browser sends, among them the one that binds a sign-in to it.
@sensitive
string Cookies

/// Who is signed in. Every other operation acts for this account.
structure Account {
    @required
    id: Id

    @required
    name: AccountName

    /// The name the account signs in with a password; absent for an account that
    /// signs in only through the OIDC provider.
    username: Username

    /// The account administers the server and changes its settings, such as the
    /// MQTT broker. The first account does.
    @required
    administrator: Boolean
}

/// How a person without a session can sign in here.
@readonly
@http(method: "GET", uri: "/manage/auth")
@auth([])
operation GetSignInOptions {
    output := {
        /// No account exists yet: the first person to sign up or sign in through
        /// the OIDC provider creates it.
        @required
        firstAccount: Boolean

        /// Whether password sign-in is enabled.
        @required
        password: Boolean

        /// The configured OIDC provider; absent when OIDC is not configured.
        oidc: OidcProvider
    }
}

structure OidcProvider {
    /// What the sign-in button calls the provider.
    @required
    name: String
}

/// Creates the first account with a password and signs it in. Later accounts come
/// from the OIDC provider.
@http(method: "POST", uri: "/manage/auth/first-account", code: 201)
@auth([])
operation CreateFirstAccount {
    input := {
        @required
        name: AccountName

        @required
        username: Username

        @required
        password: Password
    }

    output := {
        @required
        account: Account

        @httpHeader("Set-Cookie")
        session: SessionCookie
    }

    errors: [
        Conflict
    ]
}

/// Signs in with a password.
@http(method: "POST", uri: "/manage/auth/password")
@auth([])
operation SignIn {
    input := {
        @required
        username: Username

        @required
        password: Password
    }

    output := {
        @required
        account: Account

        @httpHeader("Set-Cookie")
        session: SessionCookie
    }

    errors: [
        SignInRefused
    ]
}

/// The username and password do not match an account, or password sign-in is off.
@error("client")
@httpError(401)
structure SignInRefused {
    @required
    message: String
}

/// Sends the browser to the OIDC provider, whose answer comes back to
/// `CompleteOidcSignIn`. When the provider cannot be reached, it sends the browser
/// back to the web UI with `signInError` set instead.
/// A browser redirect that sets a cookie: a 3xx code on a GET that writes state.
@suppress(["HttpResponseCodeSemantics", "HttpMethodSemantics"])
@http(method: "GET", uri: "/manage/auth/oidc", code: 302)
@auth([])
operation StartOidcSignIn {
    output := {
        @required
        @httpHeader("Location")
        location: String

        /// Binds the provider's answer to this browser; absent when the browser goes
        /// back to the web UI.
        @httpHeader("Set-Cookie")
        flow: SessionCookie
    }

    errors: [
        NotFound
    ]
}

/// The provider's redirect back. Signs the person in, creating their account on
/// first sign-in, and sends the browser to the web UI; a failure goes to the web UI
/// with `signInError` set to `refused` or `unavailable`.
/// A browser redirect that sets a cookie: a 3xx code on a GET that writes state.
@suppress(["HttpResponseCodeSemantics", "HttpMethodSemantics"])
@http(method: "GET", uri: "/manage/auth/oidc/callback", code: 302)
@auth([])
operation CompleteOidcSignIn {
    input := {
        @httpQuery("code")
        code: String

        @httpQuery("state")
        state: String

        /// Set by the provider when the person or the provider refused.
        @httpQuery("error")
        error: String

        @httpHeader("Cookie")
        cookie: Cookies
    }

    output := {
        @required
        @httpHeader("Location")
        location: String

        @httpHeader("Set-Cookie")
        session: SessionCookie
    }
}

/// Ends the session.
@http(method: "POST", uri: "/manage/auth/sign-out", code: 204)
@optionalAuth
operation SignOut {
    output := {
        /// Expires the session cookie.
        @httpHeader("Set-Cookie")
        session: SessionCookie
    }
}

@readonly
@http(method: "GET", uri: "/manage/account")
@optionalAuth
operation GetAccount {
    output := {
        @required
        account: Account
    }
}
