import { createServer, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";

import { Management } from "@afterimage/management-client";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, type MockInstance, vi } from "vitest";

import { NoAnswer, within } from "./deadline";

describe("within", () => {
  // A server that holds every request without an answer and says when it
  // holds one, or while `answers` is set answers it at once.
  const held: ServerResponse[] = [];
  let holds = () => {};
  let answers = false;
  const server = createServer((_request, response) => {
    if (answers) return void response.writeHead(204).end();
    held.push(response);
    holds();
  });
  let endpoint = "";

  // The deadline's clock is the test's: AbortSignal.timeout hands out a signal
  // the test aborts at the moment it says the time has run out.
  let deadline: AbortController;
  let timeout: MockInstance<typeof AbortSignal.timeout>;

  beforeAll(async () => {
    await new Promise<void>((listening) => server.listen(0, "127.0.0.1", listening));
    endpoint = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  });

  beforeEach(() => {
    deadline = new AbortController();
    timeout = vi.spyOn(AbortSignal, "timeout").mockImplementation(() => deadline.signal);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    answers = false;
  });

  afterAll(async () => {
    for (const response of held) response.destroy();
    await new Promise((closed) => server.close(closed));
  });

  it("ends a request the server never answers as a failure that says so", async () => {
    const client = new Management({ endpoint, maxAttempts: 1 });
    const reached = new Promise<void>((resolve) => (holds = resolve));
    const failure = within(30, (options) => client.deleteScreen({ screenId: 1 }, options));

    // The server holds the request, and only then does the time run out.
    await reached;
    deadline.abort(new DOMException("The operation timed out.", "TimeoutError"));

    await expect(failure).rejects.toBeInstanceOf(NoAnswer);
    await expect(failure).rejects.toThrow("The server did not answer within 30 seconds.");
    expect(timeout).toHaveBeenCalledWith(30_000);
    // One request was sent, and the server still holds it unanswered.
    expect(held.map((response) => response.writableEnded)).toEqual([false]);
  });

  it("lets an answer that comes in time through", async () => {
    answers = true;
    const client = new Management({ endpoint, maxAttempts: 1 });
    await expect(within(30, (options) => client.deleteScreen({ screenId: 1 }, options))).resolves.toBeDefined();
  });

  it("passes on a failure that comes in time", async () => {
    const refused = new TypeError("Failed to fetch");
    await expect(within(5, () => Promise.reject(refused))).rejects.toBe(refused);
  });
});
