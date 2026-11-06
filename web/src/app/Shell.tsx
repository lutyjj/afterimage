import { useQuery } from "@tanstack/react-query";
import { Form, Link, NavLink, Outlet, useLocation, useNavigation } from "react-router";

import { accountQuery } from "./client";
import styles from "./Shell.module.css";

/** The signed-in frame: the wordmark, the two places, and who is signed in. */
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
        </nav>
        <div className={styles.account}>
          <span className={styles.accountName}>{account?.name}</span>
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
