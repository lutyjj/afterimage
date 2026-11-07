import { useQuery } from "@tanstack/react-query";
import { NavLink, Outlet } from "react-router";

import { accountQuery } from "../app/client";
import { Page } from "../app/Page";
import styles from "./Settings.module.css";

/** Settings as a few concerns, each a route of its own: a screen's worth, and a
 * link a person can be sent. The administrator, the first account, also sees the
 * server's Home Assistant broker. */
export function SettingsPage() {
  const administrator = useQuery(accountQuery).data?.administrator ?? false;
  const sections = [
    { to: "preferences", label: "Preferences" },
    { to: "tokens", label: "Tokens and MCP" },
    ...(administrator ? [{ to: "home-assistant", label: "Home Assistant" }] : []),
    { to: "server", label: "Server" },
  ];
  return (
    <Page title="Settings" narrow>
      <nav className={styles.sections} aria-label="Settings">
        {sections.map(({ to, label }) => (
          <NavLink key={to} to={to} className={styles.section}>
            {label}
          </NavLink>
        ))}
      </nav>
      <div className={styles.content}>
        <Outlet />
      </div>
    </Page>
  );
}
