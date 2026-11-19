import { useQuery } from "@tanstack/react-query";
import { buttonClass, Card, Content, Count, EmptyState, Notice, TopBar } from "linen";
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
        {screens.isError && <Notice tone="danger" title={messageOf(screens.error)} />}
        {screens.data?.length === 0 && (
          <EmptyState
            title="No screens yet"
            description="A screen is a document: the data it reads, such as a Home Assistant sensor or any JSON over HTTP, and the components that show it in TRMNL's design. Start from an example and watch the preview update as you write."
          >
            <Link to="/screens/new" className={buttonClass("primary")}>
              Write the first screen
            </Link>
          </EmptyState>
        )}
        {screens.data && screens.data.length > 0 && (
          <Card>
            <table className="w-full border-separate border-spacing-0 text-sm">
              <thead className="text-left text-xs text-text-muted">
                <tr className="[&>th]:h-9 [&>th]:border-b [&>th]:border-border [&>th]:px-gutter [&>th]:font-medium">
                  <th scope="col">Screen</th>
                  <th scope="col">In playlists</th>
                  <th scope="col" className="max-sm:hidden">
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
                        className="relative hover:bg-sunken [&>*]:border-b [&>*]:border-border [&>*]:p-gutter last:[&>*]:border-b-0"
                      >
                        <th scope="row" className="text-left font-medium">
                          {/* The whole row answers the pointer; the name is the link. */}
                          <Link
                            to={`/screens/${screen.id}`}
                            className="after:absolute after:inset-0 hover:text-accent focus-visible:outline-none focus-visible:after:outline-2 focus-visible:after:-outline-offset-2 focus-visible:after:outline-accent"
                          >
                            {screen.name}
                          </Link>
                        </th>
                        <td className={on.length > 0 ? "text-text-secondary" : "text-text-muted"}>
                          {on.length > 0 ? on.join(", ") : "None"}
                        </td>
                        <td className="whitespace-nowrap text-text-secondary tabular-nums max-sm:hidden">
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
