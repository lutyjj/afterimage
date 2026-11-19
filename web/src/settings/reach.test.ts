import { describe, expect, it } from "vitest";

import { reachOf } from "./reach";

describe("a token's reach", () => {
  it("of a read-only token is to read and change nothing", () => {
    expect(reachOf("read_only")).toEqual({
      label: "Read only",
      description: "It can read devices, screens and playlists, and change nothing.",
    });
  });

  it("of any other token is the account's, short of its tokens and sessions", () => {
    expect(reachOf(undefined)).toEqual({
      label: "Full access",
      description: "It acts as you, except that no token can create or revoke a token or sign out.",
    });
  });
});
