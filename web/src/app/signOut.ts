import { Unauthenticated, type Management } from "@afterimage/management-client";
import type { QueryClient } from "@tanstack/react-query";
import { redirect } from "react-router";

/** The `/sign-out` route's action. Signing out is a navigation, so a page with
 * unsaved changes asks first. A session that has already ended is signed out all
 * the same; any other failure reaches the route's error page. */
export function signOutAction(client: Pick<Management, "signOut">, cache: Pick<QueryClient, "clear">) {
  return async () => {
    try {
      await client.signOut({});
    } catch (error) {
      if (!(error instanceof Unauthenticated)) throw error;
    } finally {
      cache.clear();
    }
    return redirect("/sign-in");
  };
}
