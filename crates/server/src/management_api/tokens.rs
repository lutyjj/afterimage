//! The account's personal access tokens.

use afterimage_management_api::model::{self, TokenScope};
use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, output};
use aws_smithy_types::DateTime;

use super::wire::fit;
use super::{Caller, Credential, Shared, internal};
use crate::tokens::Token;

/// A token may neither create nor revoke one, so that a leaked token cannot
/// outlive its revocation.
fn by_session(caller: &Caller, action: &str) -> Result<(), error::Forbidden> {
    match caller.credential {
        Credential::Session(_) => Ok(()),
        Credential::Token => Err(error::Forbidden {
            message: format!("A token cannot {action} tokens. Sign in to do that."),
        }),
    }
}

fn token(token: Token) -> Result<model::AccessToken, error::InternalFailure> {
    Ok(model::AccessToken {
        id: fit(token.id)?,
        name: fit(token.name)?,
        scope: token.read_only.then_some(TokenScope::ReadOnly),
        created_at: DateTime::from_secs(token.created_at),
        last_used_at: token.last_used_at.map(DateTime::from_secs),
    })
}

pub async fn create_access_token(
    input: input::CreateAccessTokenInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::CreateAccessTokenOutput, error::CreateAccessTokenError> {
    by_session(&caller, "create")?;
    let read_only = match input.scope {
        None => false,
        Some(TokenScope::ReadOnly) => true,
    };
    let (created, secret) = state
        .tokens
        .create(caller.account.id, input.name.as_str(), read_only)
        .await
        .map_err(|error| internal(&error))?;
    tracing::info!(token = created.id, read_only, "access token created");
    Ok(output::CreateAccessTokenOutput {
        token: token(created)?,
        secret: fit(secret)?,
    })
}

pub async fn list_access_tokens(
    _input: input::ListAccessTokensInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::ListAccessTokensOutput, error::ListAccessTokensError> {
    let tokens = state
        .tokens
        .list(caller.account.id)
        .await
        .map_err(|error| internal(&error))?;
    Ok(output::ListAccessTokensOutput {
        tokens: tokens.into_iter().map(token).collect::<Result<_, _>>()?,
    })
}

pub async fn revoke_access_token(
    input: input::RevokeAccessTokenInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::RevokeAccessTokenOutput, error::RevokeAccessTokenError> {
    by_session(&caller, "revoke")?;
    let revoked = state
        .tokens
        .revoke(caller.account.id, *input.token_id.inner())
        .await
        .map_err(|error| internal(&error))?;
    if !revoked {
        return Err(error::NotFound {
            message: "This token does not exist.".to_owned(),
        }
        .into());
    }
    tracing::info!(token = *input.token_id.inner(), "access token revoked");
    Ok(output::RevokeAccessTokenOutput {})
}
