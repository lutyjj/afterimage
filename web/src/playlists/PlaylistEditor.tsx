import type { Device, Playlist, ScreenSummary } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import {
  BAND_CLASS,
  Button,
  buttonClass,
  Card,
  CardBody,
  CardHead,
  CellSelect,
  classes,
  ConfirmDialog,
  Content,
  EmptyState,
  IconButton,
  LINK_CLASS,
  Nothing,
  Notice,
  SettlingItem,
  SettlingRows,
  TitleField,
  TopBar,
  useToast,
  useUnsavedChanges,
} from "linen";
import { ArrowDown, ArrowUp, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router";

import { client, messageOf, queryClient } from "../app/client";
import { ACT_SECONDS, within } from "../app/deadline";
import { SaveButton } from "../app/editing/SaveButton";
import { saveState } from "../app/editing/saveState";
import { useSaveShortcut } from "../app/editing/useSaveShortcut";
import { devicesQuery } from "../devices/queries";
import { screensQuery } from "../screens/queries";
import { playlistQuery, playlistsQuery } from "./queries";
import { playlistDeletion, type Use, usesOf } from "./usedBy";

const TRAIL = [{ href: "/playlists", label: "Playlists" }];

/** As many screens as the server keeps in a playlist. */
const MOST_SCREENS = 32;

/** A playlist's screen while it is edited, keyed so a row stays the same row
 * as it moves. */
interface Row {
  key: number;
  screenId: number;
}

let nextKey = 0;
const rowsOf = (screens: readonly number[]): Row[] => screens.map((screenId) => ({ key: nextKey++, screenId }));

/** Making or changing a playlist: its name, its screens in the order shown, and
 * the displays that show it. */
export function PlaylistEditor() {
  const { playlistId } = useParams();
  const id = playlistId === undefined ? undefined : Number(playlistId);
  const playlist = useQuery({ ...playlistQuery(id ?? 0), enabled: id !== undefined });

  if (id !== undefined && playlist.isPending) return <TopBar trail={TRAIL} title="Playlist" />;
  if (id !== undefined && playlist.isError) {
    return (
      <>
        <TopBar trail={TRAIL} title="Playlist" />
        <Content>
          <EmptyState title="This playlist could not load" description={messageOf(playlist.error)}>
            <Link to="/playlists" className={buttonClass()}>
              Back to playlists
            </Link>
          </EmptyState>
        </Content>
      </>
    );
  }
  return <Workbench key={id ?? "new"} playlist={playlist.data} />;
}

function Workbench({ playlist }: { playlist: Playlist | undefined }) {
  const navigate = useNavigate();
  const toast = useToast();
  const screens = useQuery(screensQuery).data;
  const displays = useQuery(devicesQuery);
  const devices = displays.data;
  const [name, setName] = useState(playlist?.name ?? "");
  const [rows, setRows] = useState(() => rowsOf(playlist?.screens ?? []));
  const [baseline, setBaseline] = useState({ name: playlist?.name ?? "", screens: playlist?.screens ?? [] });
  const [confirmDelete, setConfirmDelete] = useState(false);
  const chosen = rows.map((row) => row.screenId);

  const save = useMutation({
    mutationFn: async () => {
      const input = { name: name.trim(), screens: chosen };
      return playlist
        ? (await client.updatePlaylist({ playlistId: playlist.id, ...input })).playlist
        : (await client.createPlaylist(input)).playlist;
    },
    onSuccess: async (saved) => {
      setBaseline({ name: saved.name, screens: saved.screens });
      queryClient.setQueryData(playlistQuery(saved.id).queryKey, saved);
      await queryClient.invalidateQueries({ queryKey: playlistsQuery.queryKey });
      if (!playlist) void navigate(`/playlists/${saved.id}`, { replace: true });
    },
  });
  const remove = useMutation({
    mutationFn: () =>
      within(ACT_SECONDS, (options) => client.deletePlaylist({ playlistId: playlist?.id ?? 0 }, options)),
    onSuccess: () => {
      // The playlist is gone, so its own query is dropped rather than asked again.
      if (playlist) queryClient.removeQueries({ queryKey: playlistQuery(playlist.id).queryKey });
      // The dialog waits on the delete alone; the lists refresh behind it.
      void queryClient.invalidateQueries({ queryKey: playlistsQuery.queryKey });
      void queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      toast.show({ text: `${playlist?.name ?? "The playlist"} deleted` });
      void navigate("/playlists");
    },
    onError: (error) => toast.show({ text: `The playlist was not deleted. ${messageOf(error)}`, sticky: true }),
  });

  const stored = playlist !== undefined || save.isSuccess;
  const dirty = name.trim() !== baseline.name || chosen.join() !== baseline.screens.join();
  const canSave = name.trim().length > 0 && !save.isPending && (dirty || !stored);
  useSaveShortcut(() => {
    if (canSave) save.mutate();
  });
  useUnsavedChanges(dirty, { name: "this playlist", pending: save.isPending || remove.isPending });

  const move = (index: number, by: number) =>
    setRows((all) => {
      const next = [...all];
      const [row] = next.splice(index, 1);
      if (row) next.splice(index + by, 0, row);
      return next;
    });
  const add = () => {
    const unused = screens?.find((screen) => !chosen.includes(screen.id)) ?? screens?.[0];
    if (unused) setRows((all) => [...all, { key: nextKey++, screenId: unused.id }]);
  };
  // Unknown until the displays have loaded.
  const uses = playlist && devices ? usesOf(playlist.id, devices) : undefined;

  return (
    <>
      <TopBar
        trail={TRAIL}
        title={
          <TitleField
            aria-label="Playlist name"
            value={name}
            maxLength={64}
            required
            autoFocus={!playlist}
            placeholder="Name this playlist"
            onChange={(event) => setName(event.target.value)}
          />
        }
      >
        {playlist && (
          <IconButton
            variant="destructive"
            aria-label="Delete playlist"
            title="Delete playlist"
            disabled={remove.isPending}
            onClick={() => setConfirmDelete(true)}
          >
            <Trash2 aria-hidden="true" className="size-4" />
          </IconButton>
        )}
        <SaveButton
          state={saveState({ saving: save.isPending, stored, dirty })}
          disabled={!canSave}
          onSave={() => save.mutate()}
        />
      </TopBar>
      <Content>
        {save.isError && <Notice tone="danger" title={messageOf(save.error)} />}
        <Card>
          <CardHead
            title="Screens"
            description="A display shows them in this order, one at each check-in, and starts again at the first."
          >
            {screens?.length !== 0 && (
              <Button onClick={add} disabled={!screens?.length || rows.length >= MOST_SCREENS}>
                <Plus aria-hidden="true" className="size-4" /> Add a screen
              </Button>
            )}
          </CardHead>
          {screens?.length === 0 ? (
            <Nothing>
              No screens yet.{" "}
              <Link to="/screens/new" className={LINK_CLASS}>
                Write the first screen
              </Link>
              , then come back to add it here.
            </Nothing>
          ) : (
            <>
              {rows.length === 0 && (
                <Nothing>
                  Empty: a display on this playlist shows “Nothing to show”. Add a screen to give it something.
                </Nothing>
              )}
              {/* The list stays on the page while it is empty, only hidden, so
                  the first screen added arrives into the same list. */}
              <div role="list" aria-label="Screens, in order" hidden={rows.length === 0}>
                <SettlingRows scope={`playlist-${playlist?.id ?? "new"}`} marksArrivals>
                  {rows.map((row, index) => (
                    <ScreenRow
                      key={row.key}
                      row={row}
                      index={index}
                      count={rows.length}
                      screens={screens ?? []}
                      onChange={(screenId) =>
                        setRows((all) => all.map((each) => (each.key === row.key ? { ...each, screenId } : each)))
                      }
                      onMove={(by) => move(index, by)}
                      onRemove={() => setRows((all) => all.filter((each) => each.key !== row.key))}
                    />
                  ))}
                </SettlingRows>
              </div>
            </>
          )}
        </Card>
        {playlist && <ShownOn uses={uses} failure={displays.isError ? messageOf(displays.error) : undefined} />}
      </Content>
      {playlist && (
        <ConfirmDialog
          open={confirmDelete}
          onOpenChange={setConfirmDelete}
          title={`Delete ${playlist.name}?`}
          description={playlistDeletion(uses)}
          confirmLabel="Delete playlist"
          destructive
          onConfirm={() => remove.mutateAsync().then(() => setConfirmDelete(false))}
        />
      )}
    </>
  );
}

function ScreenRow({
  row,
  index,
  count,
  screens,
  onChange,
  onMove,
  onRemove,
}: {
  row: Row;
  index: number;
  count: number;
  screens: readonly ScreenSummary[];
  onChange: (screenId: number) => void;
  onMove: (by: number) => void;
  onRemove: () => void;
}) {
  // Until the screens have loaded, a saved screen is not taken for deleted.
  const known = screens.length === 0 || screens.some((screen) => screen.id === row.screenId);
  return (
    <SettlingItem
      role="listitem"
      className={classes(
        BAND_CLASS,
        "grid grid-cols-[1.25rem_minmax(0,1fr)_auto] items-center gap-x-3 sm:grid-cols-[1.5rem_minmax(0,1fr)_auto]",
      )}
    >
      <span aria-hidden="true" className="grid place-items-center text-sm font-semibold text-text-muted tabular-nums">
        {index + 1}
      </span>
      <CellSelect
        aria-label={`Screen ${index + 1}`}
        className="w-full font-medium"
        choices={[
          ...(known ? [] : [{ value: String(row.screenId), label: "A deleted screen" }]),
          ...screens.map((screen) => ({ value: String(screen.id), label: screen.name })),
        ]}
        value={String(row.screenId)}
        onValueChange={(value) => onChange(Number(value))}
      />
      <div className="flex gap-1">
        <IconButton aria-label="Move up" title="Move up" onClick={() => onMove(-1)} disabled={index === 0}>
          <ArrowUp aria-hidden="true" className="size-4" />
        </IconButton>
        <IconButton aria-label="Move down" title="Move down" onClick={() => onMove(1)} disabled={index === count - 1}>
          <ArrowDown aria-hidden="true" className="size-4" />
        </IconButton>
        <IconButton aria-label="Remove from playlist" title="Remove from playlist" onClick={onRemove}>
          <X aria-hidden="true" className="size-4" />
        </IconButton>
      </div>
    </SettlingItem>
  );
}

/** The displays that show the playlist, and when. */
function ShownOn({ uses, failure }: { uses: readonly Use[] | undefined; failure: string | undefined }) {
  return (
    <Card>
      <CardHead
        title="Shown on"
        description="A display takes a playlist in its schedule: as the one it usually shows, or for some days and hours."
      />
      {uses === undefined ? (
        failure === undefined ? (
          <Nothing>Reading displays</Nothing>
        ) : (
          <CardBody>
            <Notice tone="danger" title={<>The displays could not be read. {failure}</>} />
          </CardBody>
        )
      ) : uses.length === 0 ? (
        <Nothing>
          No display shows it yet. Choose it in a display's schedule on{" "}
          <Link to="/devices" className={LINK_CLASS}>
            Devices
          </Link>
          .
        </Nothing>
      ) : (
        <ul>
          {uses.map(({ device, when }) => (
            <UseRow key={device.id} device={device} when={when} />
          ))}
        </ul>
      )}
    </Card>
  );
}

function UseRow({ device, when }: { device: Device; when: readonly string[] }) {
  return (
    <li className={classes(BAND_CLASS, "flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 text-sm")}>
      <Link to={`/devices/${device.id}`} className="font-medium hover:text-accent">
        {device.name}
      </Link>
      <span className="text-text-secondary">{when.join(" · ")}</span>
    </li>
  );
}
