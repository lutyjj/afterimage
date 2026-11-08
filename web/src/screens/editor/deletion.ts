import type { Device, Playlist } from "@afterimage/management-client";

const names = (things: ReadonlyArray<{ name: string }>) => things.map((thing) => thing.name).join(", ");

/** What deleting a screen does to the playlists that hold it and the displays
 * that pin it or sleep on it, as the server's deletion does it. Until both lists
 * have loaded, it says what happens without naming anyone. */
export function screenDeletion(
  screenId: number,
  playlists: readonly Playlist[] | undefined,
  devices: readonly Device[] | undefined,
): string {
  if (playlists === undefined || devices === undefined) {
    return "It leaves every playlist that holds it, which may leave one with no screens; displays pinned on it go back to their schedules, and those that sleep on it sleep on Afterimage's own sleep screen instead.";
  }
  const holding = playlists.filter((playlist) => playlist.screens.includes(screenId));
  const emptied = holding.filter((playlist) => playlist.screens.every((id) => id === screenId));
  const kept = holding.filter((playlist) => !emptied.includes(playlist));
  const pinned = devices.filter((device) => device.shows.screenId === screenId);
  const sleeping = devices.filter((device) => device.sleep.screenId === screenId);
  const parts = [
    kept.length > 0 &&
      `It leaves ${kept.length === 1 ? "the playlist" : "the playlists"} ${names(kept)}, whose displays move on to their other screens.`,
    emptied.length > 0 &&
      `${names(emptied)} ${emptied.length === 1 ? "is" : "are"} left with no screens, so ${emptied.length === 1 ? "its" : "their"} displays show “Nothing to show”.`,
    pinned.length > 0 &&
      `${names(pinned)} ${pinned.length === 1 ? "goes" : "go"} back to ${pinned.length === 1 ? "its" : "their"} schedule.`,
    sleeping.length > 0 &&
      `${names(sleeping)} ${sleeping.length === 1 ? "sleeps" : "sleep"} on Afterimage's own sleep screen instead.`,
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(" ") : "No playlist or display uses it.";
}
