import type { Device } from "@afterimage/management-client";

import { hoursLabel } from "../devices/schedule";

/** A display that shows a playlist, and when: pinned, usually, or in the
 * slots that name it. */
export interface Use {
  device: Device;
  when: string[];
}

/** The displays that show the playlist, in their order, each with when. */
export function usesOf(playlistId: number, devices: readonly Device[]): Use[] {
  return devices.flatMap((device) => {
    const when = [
      ...(device.shows.playlistId === playlistId ? ["Pinned"] : []),
      ...(device.schedule.playlistId === playlistId ? ["Usually"] : []),
      ...device.schedule.slots.filter((slot) => slot.playlistId === playlistId).map(hoursLabel),
    ];
    return when.length > 0 ? [{ device, when }] : [];
  });
}
