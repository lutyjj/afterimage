import type { Device, DisplayModel } from "@afterimage/management-client";
import { queryOptions } from "@tanstack/react-query";

import { client } from "../app/client";

/** Devices check in on their own schedule, so their pages look again now and then. */
const POLL = 30_000;

export const devicesQuery = queryOptions({
  queryKey: ["devices"],
  queryFn: async () => (await client.listDevices({})).devices,
  refetchInterval: POLL,
});

export const deviceQuery = (deviceId: number) =>
  queryOptions({
    queryKey: ["devices", deviceId],
    queryFn: async () => (await client.getDevice({ deviceId })).device,
    refetchInterval: POLL,
  });

/** The device's image, which changes whenever it is drawn anew. */
export function imageUrl(device: Device): string | undefined {
  if (!device.showing) return undefined;
  return `/manage/devices/${device.id}/image?drawn=${device.showing.renderedAt.getTime()}`;
}

export function findDisplay(models: DisplayModel[] | undefined, device: Device) {
  const model = models?.find((candidate) => candidate.name === device.display.model);
  const palette = model?.palettes.find((candidate) => candidate.id === device.display.palette);
  return { model, palette };
}

/** TRMNL's firmware boards, which change with its releases every few weeks. */
export const firmwareBoardsQuery = queryOptions({
  queryKey: ["firmware-boards"],
  queryFn: async () => (await client.listFirmwareBoards({})).boards,
  staleTime: 10 * 60_000,
});
