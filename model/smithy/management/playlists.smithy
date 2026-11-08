$version: "2"

namespace afterimage.management

@length(min: 1, max: 64)
string PlaylistName

/// Screens a device shows in turn, one at each check-in, starting over after the
/// last. A device takes a playlist through its schedule (`SetSchedule`), or
/// pinned in place of it (`UpdateDevice`); one playlist serves any number of
/// devices, and one screen stands in any number of playlists.
structure Playlist {
    @required
    id: Id

    @required
    name: PlaylistName

    @required
    screens: PlaylistScreens
}

/// Screen ids in the order the playlist shows them; a screen may stand in it
/// more than once.
@length(max: 32)
list PlaylistScreens {
    member: Id
}

list Playlists {
    member: Playlist
}

/// The account's playlists, by name, with their screens.
@readonly
@http(method: "GET", uri: "/manage/playlists")
@optionalAuth
@tags(["mcp"])
operation ListPlaylists {
    output := {
        @required
        playlists: Playlists
    }
}

/// Saves a new playlist of the account's screens.
@http(method: "POST", uri: "/manage/playlists", code: 201)
@optionalAuth
@tags(["mcp"])
operation CreatePlaylist {
    input := {
        @required
        name: PlaylistName

        @required
        screens: PlaylistScreens
    }

    output := {
        @required
        playlist: Playlist
    }
}

/// The playlist with its screens.
@readonly
@http(method: "GET", uri: "/manage/playlists/{playlistId}")
@optionalAuth
@tags(["mcp"])
operation GetPlaylist {
    input := {
        @required
        @httpLabel
        playlistId: Id
    }

    output := {
        @required
        playlist: Playlist
    }

    errors: [
        NotFound
    ]
}

/// Replaces the playlist's name and screens. Every device showing it takes the
/// change at its next check-in; a change of screens starts it again at the
/// first.
@idempotent
@http(method: "PUT", uri: "/manage/playlists/{playlistId}")
@optionalAuth
@tags(["mcp"])
operation UpdatePlaylist {
    input := {
        @required
        @httpLabel
        playlistId: Id

        @required
        name: PlaylistName

        @required
        screens: PlaylistScreens
    }

    output := {
        @required
        playlist: Playlist
    }

    errors: [
        NotFound
    ]
}

/// Deletes the playlist and takes it out of every device's schedule: a device
/// that usually showed it shows nothing scheduled there, and one it was pinned on
/// goes back to its schedule.
@idempotent
@http(method: "DELETE", uri: "/manage/playlists/{playlistId}", code: 204)
@optionalAuth
@tags(["mcp"])
operation DeletePlaylist {
    input := {
        @required
        @httpLabel
        playlistId: Id
    }

    errors: [
        NotFound
    ]
}
