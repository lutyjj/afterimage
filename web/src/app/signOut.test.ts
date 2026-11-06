import { Unauthenticated } from "@afterimage/management-client";
import { describe, expect, it, vi } from "vitest";

import { signOutAction } from "./signOut";

describe("signOutAction", () => {
  const cache = () => ({ clear: vi.fn() });

  it("signs out and sends the person to sign-in", async () => {
    const client = { signOut: vi.fn().mockResolvedValue({}) };
    const cleared = cache();
    const response = (await signOutAction(client, cleared)()) as Response;
    expect(response.headers.get("Location")).toBe("/sign-in");
    expect(cleared.clear).toHaveBeenCalled();
  });

  it("treats a session that has already ended as signed out", async () => {
    const ended = new Unauthenticated({ message: "No session.", $metadata: {} });
    const client = { signOut: vi.fn().mockRejectedValue(ended) };
    const cleared = cache();
    const response = (await signOutAction(client, cleared)()) as Response;
    expect(response.headers.get("Location")).toBe("/sign-in");
    expect(cleared.clear).toHaveBeenCalled();
  });

  it("lets any other failure reach the error page", async () => {
    const client = { signOut: vi.fn().mockRejectedValue(new TypeError("Failed to fetch")) };
    const cleared = cache();
    await expect(signOutAction(client, cleared)()).rejects.toThrow("Failed to fetch");
    expect(cleared.clear).toHaveBeenCalled();
  });
});
