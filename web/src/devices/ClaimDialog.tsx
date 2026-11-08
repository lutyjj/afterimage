import { NotFound } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import { FormDialog, TextField, useToast } from "linen";
import { useId, useState } from "react";
import { useNavigate } from "react-router";

import { client, messageOf, queryClient } from "../app/client";
import { CODE_LENGTH, CodeField } from "./CodeField";
import { devicesQuery } from "./queries";

/** Claiming the display that shows a code: the person reads it off the screen,
 * often standing next to it with a phone. */
export function ClaimDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const navigate = useNavigate();
  const toast = useToast();
  const hintId = useId();
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const claim = useMutation({
    mutationFn: () => client.claimDevice({ code, name: name.trim() || undefined }),
    onSuccess: async ({ device, replaced }) => {
      await queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      change(false);
      toast.show({
        text: replaced
          ? `${device.name} set up again. It keeps its name, settings and schedule.`
          : `${device.name} claimed. It shows its first screen at its next check-in.`,
      });
      void navigate(`/devices/${device.id}`);
    },
  });
  const change = (next: boolean) => {
    onOpenChange(next);
    if (next) return;
    setCode("");
    setName("");
    claim.reset();
  };
  const notFound = claim.error instanceof NotFound;

  return (
    <FormDialog
      open={open}
      onOpenChange={change}
      title="Claim a display"
      description={<span id={hintId}>Enter the six characters the display shows.</span>}
      submitLabel={claim.isPending ? "Claiming" : "Claim display"}
      disabled={code.length < CODE_LENGTH || claim.isPending}
      onSubmit={() => claim.mutate()}
      width="26rem"
    >
      <div className="mb-4">
        <CodeField
          value={code}
          onChange={(value) => {
            setCode(value);
            if (claim.isError) claim.reset();
          }}
          label="Code on the display"
          invalid={notFound}
          describedBy={hintId}
          autoFocus
        />
      </div>
      <TextField
        label="Name, optional"
        value={name}
        maxLength={64}
        placeholder="Kitchen"
        onChange={(event) => setName(event.target.value)}
      />
      {claim.isError && (
        <p role="alert" className="-mt-2 text-sm text-danger">
          {notFound
            ? "No display shows this code. Check it on the display: a display that sets up again shows a new one."
            : messageOf(claim.error)}
        </p>
      )}
    </FormDialog>
  );
}
