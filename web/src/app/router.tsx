import { Unauthenticated } from "@afterimage/management-client";
import { createBrowserRouter, Navigate, redirect } from "react-router";

import { DeviceDetail } from "../devices/DeviceDetail";
import { DeviceList } from "../devices/DeviceList";
import { ScreenList } from "../screens/ScreenList";
import { AccessTokens } from "../settings/AccessTokens";
import { HomeAssistant } from "../settings/HomeAssistant";
import { Preferences } from "../settings/Preferences";
import { ServerInfo } from "../settings/ServerInfo";
import { SettingsPage } from "../settings/SettingsPage";
import { SignIn } from "../sign-in/SignIn";
import { accountQuery, client, queryClient, whenSignedOut } from "./client";
import { NotFoundPage, RouteError } from "./RouteError";
import { Shell } from "./Shell";
import { signInPath } from "./signInPath";
import { signOutAction } from "./signOut";

/** The signed-in account, asked of the server directly so that a missing session
 * is the loader's answer rather than an error for the whole app. */
async function account() {
  const cached = queryClient.getQueryData(accountQuery.queryKey);
  if (cached) return cached;
  try {
    const { account } = await client.getAccount({});
    queryClient.setQueryData(accountQuery.queryKey, account);
    return account;
  } catch (error) {
    if (error instanceof Unauthenticated) return undefined;
    throw error;
  }
}

/** Signed-in pages need an account; without one the person signs in first, told
 * why the provider refused when it did. */
async function requireAccount({ request }: { request: Request }) {
  const signedIn = await account();
  if (!signedIn) throw redirect(signInPath(new URL(request.url)));
  return signedIn;
}

async function signedOutOnly() {
  if (await account()) throw redirect("/");
  return null;
}

/** The page is blank while the first loader asks whether anyone is signed in, a
 * moment too short for a placeholder that would only flash. */
function Starting() {
  return null;
}

// The editor carries CodeMirror and the document schema, so it loads on first use.
const screenEditor = async () => {
  const { ScreenEditor } = await import("../screens/editor/ScreenEditor");
  return { Component: ScreenEditor };
};

export const router = createBrowserRouter([
  { path: "/sign-in", loader: signedOutOnly, Component: SignIn, HydrateFallback: Starting, ErrorBoundary: RouteError },
  { path: "/sign-out", action: signOutAction(client, queryClient), ErrorBoundary: RouteError },
  {
    path: "/",
    loader: requireAccount,
    Component: Shell,
    HydrateFallback: Starting,
    ErrorBoundary: RouteError,
    children: [
      { index: true, Component: DeviceList },
      { path: "devices/:deviceId", Component: DeviceDetail },
      { path: "screens", Component: ScreenList },
      { path: "screens/new", lazy: screenEditor },
      { path: "screens/:screenId", lazy: screenEditor },
      {
        path: "settings",
        Component: SettingsPage,
        children: [
          { index: true, element: <Navigate to="preferences" replace /> },
          { path: "preferences", Component: Preferences },
          { path: "tokens", Component: AccessTokens },
          { path: "home-assistant", Component: HomeAssistant },
          { path: "server", Component: ServerInfo },
        ],
      },
      { path: "*", Component: NotFoundPage },
    ],
  },
]);

whenSignedOut(() => {
  void router.navigate("/sign-in");
});
