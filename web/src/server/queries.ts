import { queryOptions } from "@tanstack/react-query";

import { client } from "../app/client";

/** The MQTT broker's settings, which only the administrator reads; asked again
 * every two seconds while the server connects or the broker refuses, so the
 * page shows when it gets there. */
export const mqttQuery = queryOptions({
  queryKey: ["server", "mqtt"],
  queryFn: async () => (await client.getMqttSettings({})).settings,
  refetchInterval: (query) =>
    query.state.data?.connection === "connecting" || query.state.data?.connection === "refused" ? 2000 : false,
});
