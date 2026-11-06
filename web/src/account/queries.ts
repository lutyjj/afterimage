import { queryOptions } from "@tanstack/react-query";

import { client } from "../app/client";

/** The account's personal API tokens, newest first; never their secrets. */
export const tokensQuery = queryOptions({
  queryKey: ["account", "tokens"],
  queryFn: async () => (await client.listAccessTokens({})).tokens,
});
