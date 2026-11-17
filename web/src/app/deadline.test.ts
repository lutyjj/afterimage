import { createServer, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";

import { Management } from "@afterimage/management-client";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { NoAnswer, within } from "./deadline";

describe("within", () => {
  // A server that takes every request and never answers it.
  const held: ServerResponse[] = [];
  const server = createServer((_request, response) => void held.push(response));
  let endpoint = "";

  beforeAll(async () => {
    await new Promise<void>((listening) => server.listen(0, "127.0.0.1", listening));
    endpoint = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  });

  afterAll(async () => {
    for (const response of held) response.destroy();
    await new Promise((closed) => server.close(closed));
  });

  it("ends a request the server never answers as a failure that says so", async () => {
    const client = new Management({ endpoint, maxAttempts: 1 });
    const failure = within(0.05, (options) => client.deleteScreen({ screenId: 1 }, options));
    await expect(failure).rejects.toBeInstanceOf(NoAnswer);
    await expect(failure).rejects.toThrow("The server did not answer within 0.05 seconds.");
    expect(held).toHaveLength(1);
  });

  it("passes on a failure that comes in time", async () => {
    const refused = new TypeError("Failed to fetch");
    await expect(within(5, () => Promise.reject(refused))).rejects.toBe(refused);
  });
});
