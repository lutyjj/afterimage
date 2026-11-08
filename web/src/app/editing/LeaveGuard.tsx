import { ConfirmDialog } from "linen";
import { useEffect, useRef } from "react";
import { useBlocker } from "react-router";

/** Leaving with unsaved changes asks first, inside the app and when the tab
 * closes. `detail` says what is lost. */
export function LeaveGuard({ unsaved, detail }: { unsaved: boolean; detail: string }) {
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) => unsaved && currentLocation.pathname !== nextLocation.pathname,
  );
  // Set as the person chooses to leave, so the dialog closing does not stay.
  const leaving = useRef(false);
  useEffect(() => {
    if (!unsaved) return;
    const warn = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [unsaved]);

  return (
    <ConfirmDialog
      open={blocker.state === "blocked"}
      onOpenChange={(open) => {
        if (open || leaving.current) return;
        blocker.reset?.();
      }}
      title="Leave without saving?"
      detail={detail}
      confirmLabel="Leave"
      destructive
      onConfirm={() => {
        leaving.current = true;
        blocker.proceed?.();
      }}
    />
  );
}
