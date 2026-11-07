import { useQuery } from "@tanstack/react-query";
import { AccountMenu, Button, MenuChoice, type Place, SegmentedControl, Shell as Frame } from "linen";
import { LogOut, PanelsTopLeft, Settings, Tablet } from "lucide-react";
import { Form, Outlet, useLocation, useNavigation } from "react-router";

import { accountQuery } from "./client";
import { THEME_OPTIONS, useTheme } from "./theme";
import { Mark, Wordmark } from "./Wordmark";

const PLACES: readonly Place[] = [
  { to: "/devices", label: "Devices", icon: Tablet },
  { to: "/screens", label: "Screens", icon: PanelsTopLeft },
  { to: "/settings", label: "Settings", icon: Settings },
];

/** The screen a path shows, whose change fades the next one in. Settings is
 * one screen whatever section it shows. */
function screenOf(pathname: string): string {
  return pathname.startsWith("/settings") ? "/settings" : pathname;
}

/** The signed-in frame: the rail with the places and who is signed in, a tab
 * bar on a phone. The screen editor holds the rail at its icons, so its YAML
 * and the preview at the display's own pixels share the width. */
export function Shell() {
  const { pathname } = useLocation();
  const editing = /^\/screens\/[^/]+/.test(pathname);

  return (
    <Frame
      label="Places"
      places={PLACES}
      storageKey="afterimage.rail"
      wordmark={<Wordmark />}
      mark={<Mark />}
      foot={<Account />}
      folded={editing}
      screen={screenOf(pathname)}
    >
      <Outlet />
    </Frame>
  );
}

/** Who is signed in, the theme of this browser, and the way out. */
function Account() {
  const account = useQuery(accountQuery).data;
  const { preference, setPreference } = useTheme();
  const signingOut = useNavigation().formAction === "/sign-out";
  if (!account) return null;

  return (
    <AccountMenu
      name={account.name}
      label={`Signed in as ${account.name}`}
      menuLabel="Your account"
      caption={account.username ? `Signs in as ${account.username}` : "Signs in through single sign-on"}
      footer={
        <Form method="post" action="/sign-out">
          <Button type="submit" className="w-full" disabled={signingOut}>
            <LogOut aria-hidden="true" className="size-4" />
            Sign out
          </Button>
        </Form>
      }
    >
      <MenuChoice label="Theme">
        <SegmentedControl label="Theme" value={preference} options={THEME_OPTIONS} onChange={setPreference} />
      </MenuChoice>
    </AccountMenu>
  );
}
