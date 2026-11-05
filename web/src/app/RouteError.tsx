import { isRouteErrorResponse, Link, useRouteError } from "react-router";

import { messageOf } from "./client";
import styles from "./RouteError.module.css";

/** A page that could not load, with the reason and a way back. */
export function RouteError() {
  const error = useRouteError();
  const message = isRouteErrorResponse(error)
    ? `${error.status} ${error.statusText}`
    : messageOf(error);
  return (
    <section className={styles.page}>
      <h1>This page could not load</h1>
      <p className={styles.reason}>{message}</p>
      <p>
        <a href="/">Back to devices</a>
      </p>
    </section>
  );
}

export function NotFoundPage() {
  return (
    <section className={styles.page}>
      <h1>Nothing lives at this address</h1>
      <p>
        <Link to="/">Back to devices</Link>
      </p>
    </section>
  );
}
