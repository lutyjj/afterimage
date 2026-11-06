import type { ReactNode } from "react";

import styles from "./Page.module.css";

/** A page of the app: its name, a note about it in the margin's narrow blue,
 * the actions it offers at the right, then its content. A narrow page holds
 * forms, at a measure that reads in one sweep. */
export function Page({
  title,
  note,
  actions,
  narrow = false,
  children,
}: {
  title: string;
  note?: ReactNode;
  actions?: ReactNode;
  narrow?: boolean;
  children: ReactNode;
}) {
  return (
    <div className={styles.page} data-narrow={narrow || undefined}>
      <header className={styles.head}>
        <div className={styles.titles}>
          <h1 className={styles.title}>{title}</h1>
          {note !== undefined && <p className="annotation">{note}</p>}
        </div>
        {actions}
      </header>
      {children}
    </div>
  );
}
