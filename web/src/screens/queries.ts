import { queryOptions } from "@tanstack/react-query";

import { client } from "../app/client";

export const screensQuery = queryOptions({
  queryKey: ["screens"],
  queryFn: async () => (await client.listScreens({})).screens,
});

export const screenQuery = (screenId: number) =>
  queryOptions({
    queryKey: ["screens", screenId],
    queryFn: async () => (await client.getScreen({ screenId })).screen,
  });
