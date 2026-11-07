import { ArrowLeft } from "lucide-react";
import type { ReactNode } from "react";
import { Link } from "react-router";

import styles from "./Page.module.css";

/** A page of the app: the way back to the page it belongs to, its name, a line
 * about it, the actions it offers at the right, then its content. A narrow page
 * holds forms, at a measure read in one sweep. */
export function Page({
  back,
  title,
  description,
  actions,
  narrow = false,
  children,
}: {
  back?: { to: string; label: string };
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  narrow?: boolean;
  children: ReactNode;
}) {
  return (
    <div className={styles.page} data-narrow={narrow || undefined}>
      {back && (
        <Link to={back.to} className={styles.back}>
          <ArrowLeft size={14} aria-hidden="true" />
          {back.label}
        </Link>
      )}
      <header className={styles.head}>
        <div className={styles.titles}>
          <h1 className={styles.title}>{title}</h1>
          {description !== undefined && <p className={styles.description}>{description}</p>}
        </div>
        {actions !== undefined && <div className={styles.actions}>{actions}</div>}
      </header>
      {children}
    </div>
  );
}
