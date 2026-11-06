import { describe, expect, it } from "vitest";

import { signInPath } from "./signInPath";

describe("signInPath", () => {
  it("carries the provider's refusal to the sign-in page", () => {
    expect(signInPath(new URL("https://afterimage.example/?signInError=refused"))).toBe(
      "/sign-in?signInError=refused",
    );
  });

  it("drops everything else a signed-in page was asked for", () => {
    expect(signInPath(new URL("https://afterimage.example/devices/3?tab=x"))).toBe("/sign-in");
  });
});
