import { useQuery } from "@tanstack/react-query";
import { buttonClass, Card, Content, Count, EmptyState, TopBar } from "linen";
import { Plus } from "lucide-react";
import { Link } from "react-router";

import { messageOf } from "../app/client";
import { devicesQuery } from "../devices/queries";
import { screensQuery } from "../screens/queries";
import { playlistsQuery } from "./queries";
import { usesOf } from "./usedBy";

/** Every playlist, with its screens and the displays that show it. */
export function PlaylistList() {
  const playlists = useQuery(playlistsQuery);
  const devices = useQuery(devicesQuery).data;
  const screens = useQuery(screensQuery).data;
  const screenName = (id: number) => screens?.find((screen) => screen.id === id)?.name ?? "A screen";

  return (
    <>
      <TopBar title="Playlists" meta={<Count>{playlists.data?.length}</Count>}>
        <Link to="/playlists/new" className={buttonClass("primary")}>
          <Plus aria-hidden="true" className="size-4" /> New playlist
        </Link>
      </TopBar>
      <Content>
        {playlists.isPending && <p className="text-sm text-text-muted">Reading playlists</p>}
        {playlists.isError && (
          <p role="alert" className="text-sm text-danger">
            {messageOf(playlists.error)}
          </p>
        )}
        {playlists.data?.length === 0 && (
          <EmptyState
            title="No playlists yet"
            description="A playlist is screens a display shows in turn, one at each check-in. Any number of displays can show one: as the playlist they usually show, or on the days and hours their schedule gives it."
          >
            <Link to="/playlists/new" className={buttonClass("primary")}>
              Make the first playlist
            </Link>
          </EmptyState>
        )}
        {playlists.data && playlists.data.length > 0 && (
          <Card>
            <table className="w-full border-separate border-spacing-0 text-sm">
              <thead className="text-left text-xs text-text-muted">
                <tr className="[&>th]:h-9 [&>th]:border-b [&>th]:border-border [&>th]:px-gutter [&>th]:font-medium">
                  <th scope="col">Playlist</th>
                  <th scope="col">Screens</th>
                  <th scope="col" className="max-sm:hidden">
                    Shown on
                  </th>
                </tr>
              </thead>
              <tbody>
                {playlists.data.map((playlist) => {
                  const on = usesOf(playlist.id, devices ?? []).map(({ device }) => device.name);
                  return (
                    <tr
                      key={playlist.id}
                      className="relative hover:bg-sunken [&>*]:border-b [&>*]:border-border [&>*]:p-gutter last:[&>*]:border-b-0"
                    >
                      <th scope="row" className="text-left font-medium">
                        {/* The whole row answers the pointer; the name is the link. */}
                        <Link
                          to={`/playlists/${playlist.id}`}
                          className="after:absolute after:inset-0 hover:text-accent focus-visible:outline-none focus-visible:after:outline-2 focus-visible:after:-outline-offset-2 focus-visible:after:outline-accent"
                        >
                          {playlist.name}
                        </Link>
                      </th>
                      <td className={playlist.screens.length > 0 ? "text-text-secondary" : "text-text-muted"}>
                        {playlist.screens.length > 0 ? playlist.screens.map(screenName).join(", ") : "No screens"}
                      </td>
                      <td
                        className={
                          on.length > 0 ? "text-text-secondary max-sm:hidden" : "text-text-muted max-sm:hidden"
                        }
                      >
                        {on.length > 0 ? on.join(", ") : "No display"}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Card>
        )}
      </Content>
    </>
  );
}
