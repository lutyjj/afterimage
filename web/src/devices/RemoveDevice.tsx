import type { Device } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate } from "react-router";

import { client, messageOf, queryClient } from "../app/client";
import styles from "./RemoveDevice.module.css";

type Choice = "release" | "reset";

/** Removing a device, with what each way does to the display said before it
 * happens. */
export function RemoveDevice({ device }: { device: Device }) {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [choice, setChoice] = useState<Choice>("release");
  const remove = useMutation({
    mutationFn: () => client.deleteDevice({ deviceId: device.id, reset: choice === "reset" }),
    onSuccess: async () => {
      queryClient.removeQueries({ queryKey: ["devices", device.id] });
      await queryClient.invalidateQueries({ queryKey: ["devices"] });
      void navigate("/");
    },
  });

  return (
    <section className={styles.section} aria-labelledby="remove-title">
      <h2 id="remove-title" className={styles.title}>
        Remove
      </h2>
      {!open ? (
        <div className={styles.closed}>
          <p className="hint">Take {device.name} off this server, keeping or erasing its Wi-Fi setup.</p>
          <button className="button" data-variant="danger" type="button" onClick={() => setOpen(true)}>
            Remove {device.name}…
          </button>
        </div>
      ) : (
        <div className={styles.open}>
          <fieldset className={styles.choices}>
            <legend className="visually-hidden">How to remove {device.name}</legend>
            <label className={styles.choice}>
              <input
                type="radio"
                name="remove"
                checked={choice === "release"}
                onChange={() => setChoice("release")}
              />
              <span>
                <strong>Remove</strong>
                <span className="hint">
                  At its next check-in the display shows a claim code again. It keeps its Wi-Fi and
                  this server, so anyone signed in here can claim it.
                </span>
              </span>
            </label>
            <label className={styles.choice}>
              <input type="radio" name="remove" checked={choice === "reset"} onChange={() => setChoice("reset")} />
              <span>
                <strong>Remove and reset</strong>
                <span className="hint">
                  At its next check-in the display erases its Wi-Fi networks and settings and starts
                  over at its setup portal. Someone has to set it up again at the device.
                </span>
              </span>
            </label>
          </fieldset>
          <p className="hint">Its playlist and settings go either way. Screens stay.</p>
          <div className={styles.actions}>
            <button
              className="button"
              data-variant="danger-solid"
              type="button"
              disabled={remove.isPending}
              onClick={() => remove.mutate()}
            >
              {remove.isPending ? "Removing" : choice === "reset" ? "Remove and reset" : "Remove"}
            </button>
            <button className="button" data-variant="quiet" type="button" onClick={() => setOpen(false)}>
              Keep it
            </button>
          </div>
          {remove.isError && (
            <p className="error-text" role="alert">
              {messageOf(remove.error)}
            </p>
          )}
        </div>
      )}
    </section>
  );
}
