import { useId, type ReactNode } from "react";

import styles from "./Section.module.css";

/** A part of a page on a card of its own: its title, what it is for, the actions
 * it offers, then what it holds. */
export function Section({
  title,
  intro,
  actions,
  children,
}: {
  title: string;
  intro?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className={styles.section} aria-labelledby={id}>
      <header className={styles.head}>
        <div className={styles.titles}>
          <h2 id={id} className={styles.title}>
            {title}
          </h2>
          {intro !== undefined && <p className="hint">{intro}</p>}
        </div>
        {actions !== undefined && <div className={styles.actions}>{actions}</div>}
      </header>
      <div className={styles.body}>{children}</div>
    </section>
  );
}

/** Rows of a section: each a setting or a fact with what it means, and its
 * control or value at the right. */
export function Rows({ children }: { children: ReactNode }) {
  return <div className={styles.rows}>{children}</div>;
}

export function Row({
  label,
  description,
  children,
}: {
  label: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className={styles.row}>
      <div className={styles.rowText}>
        <div className={styles.rowLabel}>{label}</div>
        {description !== undefined && <div className="hint">{description}</div>}
      </div>
      {children !== undefined && <div className={styles.rowValue}>{children}</div>}
    </div>
  );
}
