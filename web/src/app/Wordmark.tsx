/** The comet: a light and the afterimages it leaves behind, from
 * public/icon.svg, the one drawing of the icon. */
export function Mark({ className = "size-5" }: { className?: string }) {
  return <img src="/icon.svg" alt="" draggable={false} className={`shrink-0 ${className}`} />;
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
