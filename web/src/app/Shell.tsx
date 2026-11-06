import { useQuery } from "@tanstack/react-query";
import { UserRound } from "lucide-react";
import { Form, Link, NavLink, Outlet, useLocation, useNavigation } from "react-router";

import { accountQuery } from "./client";
import styles from "./Shell.module.css";

/** The signed-in frame: the wordmark, the places (the server's settings among
 * them for its administrator), and who is signed in, which leads to the
 * account's page. */
export function Shell() {
  const account = useQuery(accountQuery).data;
  const { pathname } = useLocation();
  const signingOut = useNavigation().formAction === "/sign-out";
  const onDevices = pathname === "/" || pathname.startsWith("/devices/");

  return (
    <div className={styles.shell}>
      <header className={styles.bar}>
        <Link to="/" className={styles.wordmark} aria-label="Afterimage, devices">
          Afterimage
        </Link>
        <nav className={styles.places} aria-label="Places">
          <Link to="/" className={styles.place} aria-current={onDevices ? "page" : undefined}>
            Devices
          </Link>
          <NavLink to="/screens" className={styles.place}>
            Screens
          </NavLink>
          {account?.administrator && (
            <NavLink to="/server" className={styles.place}>
              Server
            </NavLink>
          )}
        </nav>
        <div className={styles.account}>
          <NavLink to="/account" className={styles.accountLink} aria-label={`Account, ${account?.name ?? ""}`}>
            <UserRound size={16} aria-hidden="true" />
            <span className={styles.accountName}>{account?.name}</span>
          </NavLink>
          <Form method="post" action="/sign-out">
            <button type="submit" className={styles.signOut} disabled={signingOut}>
              Sign out
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
