import { useMutation, useQuery } from "@tanstack/react-query";
import { Link, NavLink, Outlet, useLocation, useNavigate } from "react-router";

import { accountQuery, client, queryClient } from "./client";
import styles from "./Shell.module.css";

/** The signed-in frame: the wordmark, the two places, and who is signed in. */
export function Shell() {
  const account = useQuery(accountQuery).data;
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const onDevices = pathname === "/" || pathname.startsWith("/devices/");
  const signOut = useMutation({
    mutationFn: () => client.signOut({}),
    onSettled: () => {
      queryClient.clear();
      void navigate("/sign-in");
    },
  });

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
          <button
            type="button"
            className={styles.signOut}
            onClick={() => signOut.mutate()}
            disabled={signOut.isPending}
          >
            Sign out
          </button>
        </div>
      </header>
      <main className={styles.main}>
        <Outlet />
      </main>
    </div>
  );
}
