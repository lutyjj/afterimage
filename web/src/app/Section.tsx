import { useId, type ReactNode } from "react";

import styles from "./Section.module.css";

/** A part of a page, opened by a hairline rule: its title, what it is for, then
 * what it holds. */
export function Section({ title, intro, children }: { title: string; intro?: ReactNode; children: ReactNode }) {
  const id = useId();
  return (
    <section className={styles.section} aria-labelledby={id}>
      <header className={styles.head}>
        <h2 id={id} className={styles.title}>
          {title}
        </h2>
        {intro !== undefined && <p className="hint">{intro}</p>}
      </header>
      {children}
    </section>
  );
}
