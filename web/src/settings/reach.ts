import type { TokenScope } from "@afterimage/management-client";

/** What a token may do, named and said in a sentence. No token may create or
 * revoke a token or sign out, whatever its reach (`service.smithy`). */
export function reachOf(scope: TokenScope | undefined): { label: string; description: string } {
  return scope === "read_only"
    ? { label: "Read only", description: "It can read devices, screens and playlists, and change nothing." }
    : {
        label: "Full access",
        description: "It acts as you, except that no token can create or revoke a token or sign out.",
      };
}
