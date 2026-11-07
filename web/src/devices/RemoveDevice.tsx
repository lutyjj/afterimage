import type { Device } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import { Button, Card, ConfirmDialog, OptionCards, SettingRow, useToast } from "linen";
import { useState } from "react";
import { useNavigate } from "react-router";

import { client, messageOf, queryClient } from "../app/client";
import { deviceQuery, devicesQuery } from "./queries";

type Choice = "release" | "reset";

/** Removing a device, with what each way does to the display said before it
 * happens. */
export function RemoveDevice({ device }: { device: Device }) {
  const navigate = useNavigate();
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const [choice, setChoice] = useState<Choice>("release");
  const remove = useMutation({
    mutationFn: (reset: boolean) => client.deleteDevice({ deviceId: device.id, reset }),
    onSuccess: async (_, reset) => {
      queryClient.removeQueries({ queryKey: deviceQuery(device.id).queryKey });
      await queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      toast.show({
        text: reset
          ? `${device.name} removed. It erases its setup at its next check-in.`
          : `${device.name} removed. It shows a claim code at its next check-in.`,
      });
      void navigate("/devices");
    },
    onError: (error) => toast.show({ text: `${device.name} was not removed. ${messageOf(error)}`, sticky: true }),
  });

  return (
    <Card>
      <SettingRow
        title={`Remove ${device.name}`}
        description="Take it off this server, keeping or erasing its Wi-Fi setup. Its playlist and settings go either way; screens stay."
      >
        <Button
          variant="destructive"
          disabled={remove.isPending}
          onClick={() => {
            setChoice("release");
            setOpen(true);
          }}
        >
          {remove.isPending ? "Removing" : "Remove"}
        </Button>
      </SettingRow>
      <ConfirmDialog
        open={open}
        onOpenChange={setOpen}
        title={`Remove ${device.name}?`}
        detail="Its playlist and settings go either way. Screens stay."
        confirmLabel={choice === "reset" ? "Remove and reset" : "Remove"}
        destructive
        width="34rem"
        onConfirm={() => remove.mutate(choice === "reset")}
      >
        <OptionCards
          label={`How to remove ${device.name}`}
          value={choice}
          onChoose={setChoice}
          options={[
            {
              value: "release",
              label: "Remove",
              detail:
                "At its next check-in it shows a claim code again. It keeps its Wi-Fi and this server, so anyone signed in here can claim it.",
            },
            {
              value: "reset",
              label: "Remove and reset",
              detail:
                "At its next check-in it erases its Wi-Fi networks and settings and starts over at its setup portal. Someone has to set it up again at the device.",
            },
          ]}
        />
      </ConfirmDialog>
    </Card>
  );
}
