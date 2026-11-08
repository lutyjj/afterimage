//! Playlists: screens any number of devices show in turn.

use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, model, output};

use super::{Caller, Shared, internal, invalid, wire};
use crate::playlists::Error;

fn not_found() -> error::NotFound {
    error::NotFound {
        message: "This playlist does not exist.".to_owned(),
    }
}

/// Screen ids as the store keeps them.
fn screens(screens: &model::PlaylistScreens) -> Vec<i64> {
    screens.inner().iter().map(|id| *id.inner()).collect()
}

/// A screen the account does not have is the caller's mistake.
fn refused(error: &Error) -> error::ValidationException {
    invalid(error.to_string())
}

pub async fn list_playlists(
    _input: input::ListPlaylistsInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::ListPlaylistsOutput, error::ListPlaylistsError> {
    let playlists = state
        .playlists
        .list(caller.account.id)
        .await
        .map_err(|error| internal(&error))?;
    Ok(output::ListPlaylistsOutput {
        playlists: playlists
            .into_iter()
            .map(wire::playlist)
            .collect::<Result<_, _>>()?,
    })
}

pub async fn create_playlist(
    input: input::CreatePlaylistInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::CreatePlaylistOutput, error::CreatePlaylistError> {
    let playlist = match state
        .playlists
        .create(
            caller.account.id,
            input.name.as_str(),
            &screens(&input.screens),
        )
        .await
    {
        Ok(playlist) => playlist,
        Err(error @ Error::UnknownScreen(_)) => return Err(refused(&error).into()),
        Err(error) => return Err(internal(&error).into()),
    };
    // Playlists are options of each device's "Showing" control.
    state.telemetry.options_changed();
    Ok(output::CreatePlaylistOutput {
        playlist: wire::playlist(playlist)?,
    })
}

pub async fn get_playlist(
    input: input::GetPlaylistInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::GetPlaylistOutput, error::GetPlaylistError> {
    let playlist = state
        .playlists
        .get(caller.account.id, *input.playlist_id.inner())
        .await
        .map_err(|error| internal(&error))?
        .ok_or_else(not_found)?;
    Ok(output::GetPlaylistOutput {
        playlist: wire::playlist(playlist)?,
    })
}

pub async fn update_playlist(
    input: input::UpdatePlaylistInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::UpdatePlaylistOutput, error::UpdatePlaylistError> {
    let playlist = match state
        .playlists
        .update(
            caller.account.id,
            *input.playlist_id.inner(),
            input.name.as_str(),
            &screens(&input.screens),
        )
        .await
    {
        Ok(playlist) => playlist.ok_or_else(not_found)?,
        Err(error @ Error::UnknownScreen(_)) => return Err(refused(&error).into()),
        Err(error) => return Err(internal(&error).into()),
    };
    // Playlists are options of each device's "Showing" control, by name.
    state.telemetry.options_changed();
    Ok(output::UpdatePlaylistOutput {
        playlist: wire::playlist(playlist)?,
    })
}

pub async fn delete_playlist(
    input: input::DeletePlaylistInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::DeletePlaylistOutput, error::DeletePlaylistError> {
    let deleted = state
        .playlists
        .delete(caller.account.id, *input.playlist_id.inner())
        .await
        .map_err(|error| internal(&error))?;
    if !deleted {
        return Err(not_found().into());
    }
    // Playlists are options of each device's "Showing" control, and a device
    // pinned on this one is back on its schedule.
    state.telemetry.options_changed();
    Ok(output::DeletePlaylistOutput {})
}
