import { queryOptions } from "@tanstack/react-query";

import { client } from "../app/client";

export const playlistsQuery = queryOptions({
  queryKey: ["playlists"],
  queryFn: async () => (await client.listPlaylists({})).playlists,
});

export const playlistQuery = (playlistId: number) =>
  queryOptions({
    queryKey: ["playlists", playlistId],
    queryFn: async () => (await client.getPlaylist({ playlistId })).playlist,
  });
