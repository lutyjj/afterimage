import { Chip, classes, WarnDot } from "linen";

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
    <span
      title={title}
      className={classes("inline-flex [transition:opacity_var(--motion-state)]", ended && "line-through opacity-55")}
    >
      <Chip size="row" tone={ended ? "neutral" : "warn"}>
        {!ended && <WarnDot />}
        {children}
        {ended && <span className="sr-only"> (no longer)</span>}
      </Chip>
    </span>
  );
}
