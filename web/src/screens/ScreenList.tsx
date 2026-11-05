import { useQuery } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { Link } from "react-router";

import { messageOf } from "../app/client";
import { ago } from "../app/format";
import { devicesQuery } from "../devices/queries";
import { screensQuery } from "./queries";
import styles from "./ScreenList.module.css";

/** Every screen, with the displays that show it. */
export function ScreenList() {
  const screens = useQuery(screensQuery);
  const devices = useQuery(devicesQuery).data;
  const shownOn = (id: number) =>
    devices?.filter((device) => device.playlist.some((entry) => entry.screenId === id)).map((device) => device.name) ??
    [];

  return (
    <div className={styles.page}>
      <header className={styles.head}>
        <h1 className={styles.heading}>Screens</h1>
        <Link to="/screens/new" className="button" data-variant="primary">
          <Plus size={16} aria-hidden="true" /> New screen
        </Link>
      </header>
      {screens.isPending && <p className="annotation">Loading screens</p>}
      {screens.isError && <p className="error-text">{messageOf(screens.error)}</p>}
      {screens.data?.length === 0 && (
        <div className={styles.empty}>
          <h2>No screens yet</h2>
          <p>
            A screen is a document: the data it reads, such as a Home Assistant sensor or any JSON
            over HTTP, and the components that show it in TRMNL's design. Start from an example
            and watch the proof update as you write.
          </p>
          <Link to="/screens/new" className="button" data-variant="primary">
            Write the first screen
          </Link>
        </div>
      )}
      {screens.data && screens.data.length > 0 && (
        <table className={styles.table}>
          <thead>
            <tr>
              <th scope="col">Screen</th>
              <th scope="col">Shown on</th>
              <th scope="col">Changed</th>
            </tr>
          </thead>
          <tbody>
            {[...screens.data]
              .sort((a, b) => a.name.localeCompare(b.name))
              .map((screen) => {
                const on = shownOn(screen.id);
                return (
                  <tr key={screen.id}>
                    <th scope="row">
                      <Link to={`/screens/${screen.id}`}>{screen.name}</Link>
                    </th>
                    <td className={on.length === 0 ? styles.none : undefined}>
                      {on.length > 0 ? on.join(", ") : "No display"}
                    </td>
                    <td className={styles.when}>{ago(screen.updatedAt)}</td>
                  </tr>
                );
              })}
          </tbody>
        </table>
      )}
    </div>
  );
}
