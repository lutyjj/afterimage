import { describe, expect, it } from "vitest";

import { sourcesNamed } from "./sourceProblems";

describe("the sources problems name", () => {
  it("are those a problem's path names as a whole", () => {
    expect(
      sourcesNamed([
        { path: "/sources/weather" },
        { path: "/sources/bus/url" },
        { path: "/content/column" },
        { path: undefined },
        {},
      ]),
    ).toEqual(new Set(["weather"]));
  });

  it("read a name back from its escaped pointer segment", () => {
    expect(sourcesNamed([{ path: "/sources/a~1b~0c" }])).toEqual(new Set(["a/b~c"]));
  });
});
