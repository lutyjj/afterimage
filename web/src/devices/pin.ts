import type { Shows } from "@afterimage/management-client";

/** What a display shows while awake as the "Showing" picker's value: "" for
 * its schedule, `playlist:<id>` or `screen:<id>` for what is pinned. */
export function pinValue(shows: Shows): string {
  if (shows.playlistId !== undefined) return `playlist:${shows.playlistId}`;
  if (shows.screenId !== undefined) return `screen:${shows.screenId}`;
  return "";
}

/** The picker's value as what the display shows; anything else is its schedule. */
export function pinOf(value: string): Shows {
  const [, kind, id] = /^(playlist|screen):(\d+)$/.exec(value) ?? [];
  if (kind === "playlist") return { playlistId: Number(id) };
  if (kind === "screen") return { screenId: Number(id) };
  return { schedule: {} };
}
