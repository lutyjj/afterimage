//! Signing in and out, and the signed-in account.

use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, model, output};

use super::{Caller, Credential, OverTls, Shared, internal, wire};
use crate::oidc;
use crate::sessions::{CookieName, SESSION_SECONDS};

pub async fn get_sign_in_options(
    _input: input::GetSignInOptionsInput,
    Extension(state): Shared,
) -> Result<output::GetSignInOptionsOutput, error::GetSignInOptionsError> {
    let exist = state
        .accounts
        .exist()
        .await
        .map_err(|error| internal(&error))?;
    Ok(output::GetSignInOptionsOutput {
        first_account: !exist,
        password: state.password_sign_in,
        oidc: state.oidc.as_ref().map(|oidc| model::OidcProvider {
            name: oidc.name().to_owned(),
        }),
    })
}

pub async fn create_first_account(
    input: input::CreateFirstAccountInput,
    Extension(state): Shared,
    OverTls(secure): OverTls,
) -> Result<output::CreateFirstAccountOutput, error::CreateFirstAccountError> {
    if !state.password_sign_in {
        return Err(error::Conflict {
            message: "Password sign-in is off; sign in through the OIDC provider.".to_owned(),
        }
        .into());
    }
    let account = state
        .accounts
        .create_first(
            input.name.as_str(),
            input.username.as_str(),
            input.password.into_inner(),
        )
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(|| error::Conflict {
            message: "An account exists already; sign in instead.".to_owned(),
        })?;
    let token = state
        .sessions
        .start(account.id)
        .await
        .map_err(|error| internal(&error))?;
    tracing::info!(account = account.id, "first account created");
    Ok(output::CreateFirstAccountOutput {
        account: wire::account(account)?,
        session: Some(CookieName::Session.set(token, SESSION_SECONDS, secure)),
    })
}

pub async fn sign_in(
    input: input::SignInInput,
    Extension(state): Shared,
    OverTls(secure): OverTls,
) -> Result<output::SignInOutput, error::SignInError> {
    let refused = || error::SignInRefused {
        message: "The username or password is wrong.".to_owned(),
    };
    if !state.password_sign_in {
        return Err(refused().into());
    }
    let account = state
        .accounts
        .with_password(input.username.as_str(), input.password.into_inner())
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(refused)?;
    let token = state
        .sessions
        .start(account.id)
        .await
        .map_err(|error| internal(&error))?;
    Ok(output::SignInOutput {
        account: wire::account(account)?,
        session: Some(CookieName::Session.set(token, SESSION_SECONDS, secure)),
    })
}

pub async fn start_oidc_sign_in(
    _input: input::StartOidcSignInInput,
    Extension(state): Shared,
    OverTls(secure): OverTls,
) -> Result<output::StartOidcSignInOutput, error::StartOidcSignInError> {
    let oidc = state.oidc.as_ref().ok_or_else(|| error::NotFound {
        message: "OIDC sign-in is not configured.".to_owned(),
    })?;
    Ok(match oidc.start(secure).await {
        Ok(started) => output::StartOidcSignInOutput {
            location: started.location,
            flow: Some(started.cookie),
        },
        Err(error) => {
            tracing::warn!(%error, "OIDC sign-in could not start");
            output::StartOidcSignInOutput {
                location: failed(&state.landing, &error),
                flow: None,
            }
        }
    })
}

pub async fn complete_oidc_sign_in(
    input: input::CompleteOidcSignInInput,
    Extension(state): Shared,
    OverTls(secure): OverTls,
) -> Result<output::CompleteOidcSignInOutput, error::CompleteOidcSignInError> {
    let refuse = |error: oidc::Error| {
        tracing::warn!(%error, "OIDC sign-in failed");
        output::CompleteOidcSignInOutput {
            location: failed(&state.landing, &error),
            session: None,
        }
    };
    let Some(oidc) = state.oidc.as_ref() else {
        return Ok(refuse(oidc::Error::Refused));
    };
    let binding = input
        .cookie
        .as_deref()
        .and_then(|cookie| CookieName::OidcFlow.read(cookie, secure));
    let (Some(code), Some(flow_state), Some(binding), None) =
        (input.code, input.state, binding, input.error)
    else {
        return Ok(refuse(oidc::Error::Refused));
    };
    let identity = match oidc.complete(code, &flow_state, &binding).await {
        Ok(identity) => identity,
        Err(error) => return Ok(refuse(error)),
    };
    let account = state
        .accounts
        .with_oidc(&identity.issuer, &identity.subject, &identity.name)
        .await
        .map_err(|error| internal(&error))?;
    let token = state
        .sessions
        .start(account.id)
        .await
        .map_err(|error| internal(&error))?;
    Ok(output::CompleteOidcSignInOutput {
        location: state.landing.clone(),
        session: Some(CookieName::Session.set(token, SESSION_SECONDS, secure)),
    })
}

/// The web UI, told why sign-in failed.
fn failed(landing: &str, error: &oidc::Error) -> String {
    let reason = match error {
        oidc::Error::Refused => "refused",
        oidc::Error::Unavailable | oidc::Error::Storage(_) => "unavailable",
    };
    format!("{landing}?signInError={reason}")
}

pub async fn sign_out(
    _input: input::SignOutInput,
    Extension(state): Shared,
    caller: Caller,
    OverTls(secure): OverTls,
) -> Result<output::SignOutOutput, error::SignOutError> {
    let Credential::Session(token) = &caller.credential else {
        return Err(error::Forbidden {
            message: "A token has no session to end. Revoke the token instead.".to_owned(),
        }
        .into());
    };
    state
        .sessions
        .end(token)
        .await
        .map_err(|error| internal(&error))?;
    Ok(output::SignOutOutput {
        session: Some(CookieName::Session.clear(secure)),
    })
}

pub async fn get_account(
    _input: input::GetAccountInput,
    caller: Caller,
) -> Result<output::GetAccountOutput, error::GetAccountError> {
    Ok(output::GetAccountOutput {
        account: wire::account(caller.account)?,
    })
}
