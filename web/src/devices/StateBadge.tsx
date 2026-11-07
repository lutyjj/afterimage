import styles from "./StateBadge.module.css";

export interface StateBadgeProps {
  children: string;
  /** The state has ended while the page was open: the badge stays, struck
   * through and faded, rather than vanishing under the reader's eye. */
  ended?: boolean;
  title?: string;
}

/** A state of a device that asks for the person's attention. */
export function StateBadge({ children, ended = false, title }: StateBadgeProps) {
  return (
    <span className={styles.badge} data-ended={ended || undefined} title={title}>
      {children}
      {ended && <span className="visually-hidden"> (no longer)</span>}
    </span>
  );
}
