import styles from "./Stamp.module.css";

export interface StampProps {
  children: string;
  /** The state has ended: the stamp stays, struck through, rather than vanishing. */
  cancelled?: boolean;
  title?: string;
}

/** A device's state, stamped in proofer's red. */
export function Stamp({ children, cancelled = false, title }: StampProps) {
  return (
    <span className={styles.stamp} data-cancelled={cancelled || undefined} title={title}>
      {children}
      {cancelled && <span className="visually-hidden"> (no longer)</span>}
    </span>
  );
}
