/** A square and the fainter square it leaves behind, as an e-paper screen keeps
 * the last image it showed. */
export function Mark({ className = "size-4" }: { className?: string }) {
  return (
    <svg className={`shrink-0 ${className}`} viewBox="0 0 16 16" aria-hidden="true">
      <rect
        x="5.5"
        y="5.5"
        width="9"
        height="9"
        rx="1.5"
        className="fill-none stroke-[color-mix(in_oklch,var(--color-ink)_45%,transparent)] stroke-[1.25]"
      />
      <rect x="1.5" y="1.5" width="9" height="9" rx="1.5" className="fill-ink" />
    </svg>
  );
}

/** The name, after its mark. */
export function Wordmark() {
  return (
    <span className="inline-flex items-center gap-2 font-display text-md font-semibold tracking-[-0.02em] text-text">
      <Mark />
      afterimage
    </span>
  );
}
