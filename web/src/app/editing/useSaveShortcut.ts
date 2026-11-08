import { useEffect, useRef } from "react";

/** Ctrl+S or Cmd+S saves, wherever focus is, instead of the browser saving the
 * page. `save` runs as it stands at the press. */
export function useSaveShortcut(save: () => void) {
  const latest = useRef(save);
  latest.current = save;
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "s") {
        event.preventDefault();
        latest.current();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
