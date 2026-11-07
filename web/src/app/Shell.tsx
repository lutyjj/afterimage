import { useQuery } from "@tanstack/react-query";
import { LogOut, PanelsTopLeft, Settings, Tablet } from "lucide-react";
import type { ReactNode } from "react";
import { Form, Link, NavLink, Outlet, useLocation, useNavigation } from "react-router";

import { accountQuery } from "./client";
import styles from "./Shell.module.css";
import { Wordmark } from "./Wordmark";

/** The signed-in frame: the wordmark, the places, and who is signed in. The
 * screen editor folds the rail to its icons, so its YAML and the preview at the
 * display's own pixels share the width; on a phone the rail is a bar along the
 * top. */
export function Shell() {
  const account = useQuery(accountQuery).data;
  const { pathname } = useLocation();
  const signingOut = useNavigation().formAction === "/sign-out";
  const compact = /^\/screens\/[^/]+/.test(pathname);

  return (
    <div className={styles.shell} data-compact={compact || undefined}>
      <header className={styles.rail}>
        <Link to="/" className={styles.home} aria-label="Afterimage, devices">
          <Wordmark />
        </Link>
        <nav className={styles.places} aria-label="Places">
          <Place to="/" icon={<Tablet />} current={pathname === "/" || pathname.startsWith("/devices/")}>
            Devices
          </Place>
          <Place to="/screens" icon={<PanelsTopLeft />} current={pathname.startsWith("/screens")}>
            Screens
          </Place>
          <Place to="/settings" icon={<Settings />} current={pathname.startsWith("/settings")}>
            Settings
          </Place>
        </nav>
        <div className={styles.account}>
          <span className={styles.initial} aria-hidden="true">
            {account?.name.trim().charAt(0).toUpperCase()}
          </span>
          <span className={styles.name}>{account?.name}</span>
          <Form method="post" action="/sign-out">
            <button type="submit" className="icon-button" disabled={signingOut} aria-label="Sign out" title="Sign out">
              <LogOut size={16} aria-hidden="true" />
            </button>
          </Form>
        </div>
      </header>
      <main className={styles.main}>
        <Outlet />
      </main>
    </div>
  );
}

function Place({ to, icon, current, children }: { to: string; icon: ReactNode; current: boolean; children: string }) {
  return (
    <NavLink to={to} className={styles.place} aria-current={current ? "page" : undefined} title={children}>
      <span className={styles.icon} aria-hidden="true">
        {icon}
      </span>
      <span className={styles.label}>{children}</span>
    </NavLink>
  );
}
