/** How long an act a person waits on gives the server. Its writes answer in well
 * under a second, and stopping MQTT publishing waits at most a few seconds for
 * the broker. */
export const ACT_SECONDS = 30;

/** The server gave no answer within the time the act allowed it. */
export class NoAnswer extends Error {
  constructor(seconds: number) {
    super(`The server did not answer within ${seconds} seconds.`);
    this.name = "NoAnswer";
  }
}

/** Runs a request that stops waiting for the server after `seconds`, so an act
 * a dialog waits on ends as a failure the person sees instead of never. */
export async function within<T>(
  seconds: number,
  request: (options: { abortSignal: AbortSignal }) => Promise<T>,
): Promise<T> {
  const abortSignal = AbortSignal.timeout(seconds * 1000);
  try {
    return await request({ abortSignal });
  } catch (error) {
    // The client's error for an aborted request differs between runtimes, so
    // the signal says whether the time ran out.
    if (abortSignal.aborted) throw new NoAnswer(seconds);
    throw error;
  }
}
