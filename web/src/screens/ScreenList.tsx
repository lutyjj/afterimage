import { useQuery } from "@tanstack/react-query";
import { buttonClass, Card, Content, Count, EmptyState, TopBar } from "linen";
import { Plus } from "lucide-react";
import { Link } from "react-router";

import { messageOf } from "../app/client";
import { ago } from "../app/format";
import { playlistsQuery } from "../playlists/queries";
import { screensQuery } from "./queries";

/** Every screen, with the playlists that hold it. */
export function ScreenList() {
  const screens = useQuery(screensQuery);
  const playlists = useQuery(playlistsQuery).data;
  const shownOn = (id: number) =>
    playlists?.filter((playlist) => playlist.screens.includes(id)).map((playlist) => playlist.name) ?? [];

  return (
    <>
      <TopBar title="Screens" meta={<Count>{screens.data?.length}</Count>}>
        <Link to="/screens/new" className={buttonClass("primary")}>
          <Plus aria-hidden="true" className="size-4" /> New screen
        </Link>
      </TopBar>
      <Content>
        {screens.isPending && <p className="text-sm text-text-muted">Reading screens</p>}
        {screens.isError && (
          <p role="alert" className="text-sm text-danger">
            {messageOf(screens.error)}
          </p>
        )}
        {screens.data?.length === 0 && (
          <EmptyState
            title="No screens yet"
            detail="A screen is a document: the data it reads, such as a Home Assistant sensor or any JSON over HTTP, and the components that show it in TRMNL's design. Start from an example and watch the preview update as you write."
          >
            <Link to="/screens/new" className={buttonClass("primary")}>
              Write the first screen
            </Link>
          </EmptyState>
        )}
        {screens.data && screens.data.length > 0 && (
          <Card>
            <table className="w-full border-collapse text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs text-text-muted">
                  <th scope="col" className="px-gutter py-2.5 font-medium">
                    Screen
                  </th>
                  <th scope="col" className="px-gutter py-2.5 font-medium">
                    In playlists
                  </th>
                  <th scope="col" className="px-gutter py-2.5 font-medium">
                    Changed
                  </th>
                </tr>
              </thead>
              <tbody>
                {[...screens.data]
                  .sort((a, b) => a.name.localeCompare(b.name))
                  .map((screen) => {
                    const on = shownOn(screen.id);
                    return (
                      <tr
                        key={screen.id}
                        className="relative h-(--height-row) border-b border-border last:border-b-0 hover:bg-sunken"
                      >
                        <th scope="row" className="px-gutter text-left font-medium">
                          {/* The whole row answers the pointer; the name is the link. */}
                          <Link
                            to={`/screens/${screen.id}`}
                            className="after:absolute after:inset-0 hover:text-accent focus-visible:outline-none focus-visible:after:outline-2 focus-visible:after:-outline-offset-2 focus-visible:after:outline-accent"
                          >
                            {screen.name}
                          </Link>
                        </th>
                        <td className={on.length > 0 ? "px-gutter text-text-secondary" : "px-gutter text-text-muted"}>
                          {on.length > 0 ? on.join(", ") : "None"}
                        </td>
                        <td className="px-gutter whitespace-nowrap text-text-secondary tabular-nums">
                          {ago(screen.updatedAt)}
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
