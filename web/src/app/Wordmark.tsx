import styles from "./Wordmark.module.css";

/** A square and the fainter square it leaves behind, as an e-paper screen keeps
 * the last image it showed. */
export function Mark({ className }: { className?: string }) {
  return (
    <svg className={`${styles.mark} ${className ?? ""}`} viewBox="0 0 16 16" aria-hidden="true">
      <rect x="5.5" y="5.5" width="9" height="9" rx="1.5" className={styles.after} />
      <rect x="1.5" y="1.5" width="9" height="9" rx="1.5" className={styles.image} />
    </svg>
  );
}

/** The name, after its mark. */
export function Wordmark() {
  return (
    <span className={styles.wordmark}>
      <Mark />
      <span data-wordmark-name>afterimage</span>
    </span>
  );
}
