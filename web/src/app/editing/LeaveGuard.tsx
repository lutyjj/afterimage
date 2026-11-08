import { ConfirmDialog } from "linen";
import { useEffect, useRef } from "react";
import { useBlocker } from "react-router";

/** Leaving with unsaved changes asks first, inside the app and when the tab
 * closes. While the editor's own save or delete is under way (`settling`), it
 * may move to another page without asking, as it does once that ends; closing
 * the tab still asks, since the save may yet fail. `detail` says what is lost. */
export function LeaveGuard({ unsaved, settling, detail }: { unsaved: boolean; settling: boolean; detail: string }) {
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) => unsaved && !settling && currentLocation.pathname !== nextLocation.pathname,
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
