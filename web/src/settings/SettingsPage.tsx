import { useQuery } from "@tanstack/react-query";
import { Content, type Section, SectionNav, TopBar } from "linen";
import { Outlet } from "react-router";

import { accountQuery } from "../app/client";

/** Settings as a few concerns, each a route of its own: a screen's worth, and a
 * link a person can be sent. The administrator, the first account, also sees the
 * server's Home Assistant broker. */
export function SettingsPage() {
  const administrator = useQuery(accountQuery).data?.administrator ?? false;
  const sections: Section[] = [
    { to: "/settings/preferences", label: "Preferences" },
    { to: "/settings/tokens", label: "Tokens and MCP" },
    ...(administrator ? [{ to: "/settings/home-assistant", label: "Home Assistant" }] : []),
    { to: "/settings/server", label: "Server" },
  ];
  return (
    <>
      <TopBar title="Settings" />
      <SectionNav label="Settings" sections={sections} />
      <Content>
        <div className="grid gap-4">
          <Outlet />
        </div>
      </Content>
    </>
  );
}
