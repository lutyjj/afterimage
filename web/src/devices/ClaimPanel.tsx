import { NotFound } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import { useId, useState, type FormEvent } from "react";
import { useNavigate } from "react-router";

import { client, messageOf, queryClient } from "../app/client";
import { CODE_LENGTH, CodeField } from "./CodeField";
import styles from "./ClaimPanel.module.css";
import { devicesQuery } from "./queries";

/** Claiming the display that shows a code: the person reads it off the screen. */
export function ClaimPanel() {
  const navigate = useNavigate();
  const hintId = useId();
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const claim = useMutation({
    mutationFn: () => client.claimDevice({ code, name: name.trim() || undefined }),
    onSuccess: async ({ device, replaced }) => {
      await queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      void navigate(`/devices/${device.id}`, { state: { claimed: true, replaced } });
    },
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (code.length === CODE_LENGTH) claim.mutate();
  };
  const notFound = claim.error instanceof NotFound;

  return (
    <form className={styles.panel} onSubmit={submit} aria-labelledby={`${hintId}-title`}>
      <h2 id={`${hintId}-title`} className={styles.title}>
        Claim a display
      </h2>
      <p id={hintId} className="hint">
        Enter the six characters the display shows.
      </p>
      <CodeField
        value={code}
        onChange={(value) => {
          setCode(value);
          if (claim.isError) claim.reset();
        }}
        label="Code on the display"
        invalid={notFound}
        describedBy={hintId}
      />
      <label className="field">
        <span>Name, optional</span>
        <input
          className="input"
          value={name}
          maxLength={64}
          placeholder="Kitchen"
          onChange={(event) => setName(event.target.value)}
        />
      </label>
      {claim.isError && (
        <p className="error-text" role="alert">
          {notFound
            ? "No display shows this code. Check it on the display: a display that sets up again shows a new one."
            : messageOf(claim.error)}
        </p>
      )}
      <button
        className="button"
        data-variant="primary"
        type="submit"
        disabled={code.length < CODE_LENGTH || claim.isPending}
      >
        {claim.isPending ? "Claiming" : "Claim display"}
      </button>
    </form>
  );
}
