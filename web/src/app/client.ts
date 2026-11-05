import {
  Conflict,
  Management,
  ManagementServiceException,
  NotFound,
  SignInRefused,
  Unauthenticated,
  ValidationException,
} from "@afterimage/management-client";
import { QueryCache, MutationCache, QueryClient, queryOptions } from "@tanstack/react-query";

/** The management API at this page's origin. The session cookie rides along on
 * every same-origin request; a failed request is never retried behind the
 * person's back. */
export const client = new Management({
  endpoint: window.location.origin,
  maxAttempts: 1,
});

let onSignedOut: () => void = () => {};

/** What to do when a request finds the session gone: the router sends the person
 * to sign in. */
export function whenSignedOut(callback: () => void) {
  onSignedOut = callback;
}

/** A session that ends while the app is open sends the person to sign in. */
function signedOut(error: unknown) {
  if (error instanceof Unauthenticated && window.location.pathname !== "/sign-in") {
    queryClient.clear();
    onSignedOut();
  }
}

export const queryClient = new QueryClient({
  queryCache: new QueryCache({ onError: signedOut }),
  mutationCache: new MutationCache({ onError: signedOut }),
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: 10_000,
      refetchOnWindowFocus: true,
    },
  },
});

/** A sentence for the person about why a request failed. */
export function messageOf(error: unknown): string {
  if (error instanceof ValidationException) {
    const fields = error.fieldList?.map((field) => field.message) ?? [];
    return fields.length > 0 ? fields.join(" ") : error.message;
  }
  if (
    error instanceof NotFound ||
    error instanceof Conflict ||
    error instanceof SignInRefused ||
    error instanceof Unauthenticated
  ) {
    return error.message;
  }
  if (error instanceof ManagementServiceException) {
    return `The server could not do that: ${error.message}`;
  }
  if (error instanceof TypeError) {
    return "The server did not answer. Check that it is running and reachable.";
  }
  return error instanceof Error ? error.message : String(error);
}

export const signInOptionsQuery = queryOptions({
  queryKey: ["sign-in-options"],
  queryFn: () => client.getSignInOptions({}),
  staleTime: Infinity,
});

export const accountQuery = queryOptions({
  queryKey: ["account"],
  queryFn: async () => (await client.getAccount({})).account,
  staleTime: Infinity,
});

export const serverQuery = queryOptions({
  queryKey: ["server"],
  queryFn: () => client.getServer({}),
  staleTime: Infinity,
});

export const displayModelsQuery = queryOptions({
  queryKey: ["display-models"],
  queryFn: async () => (await client.listDisplayModels({})).models,
  staleTime: Infinity,
});
