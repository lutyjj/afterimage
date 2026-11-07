import { ValidationException, type Display, type Problem, type ScreenDocument, type SourceResult } from "@afterimage/management-client";
import { useEffect, useState } from "react";

import { client, messageOf } from "../../app/client";

export interface Preview {
  /** A data URL of the last image drawn; kept while the next one draws. It is
   * a value rather than a handle to release, so a panel may keep showing it as
   * the image fades out. */
  image?: string;
  sources?: Record<string, SourceResult>;
  /** What kept the screen from being drawn as written, with document paths. */
  problems: Problem[];
  drawing: boolean;
  /** The image is from an earlier document: the latest one could not be drawn. */
  stale: boolean;
  /** The preview request itself failed, such as the server being away. */
  failure?: string;
}

const WAIT = 600;

function dataUrl(png: Uint8Array): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error ?? new Error("the image could not be read"));
    reader.readAsDataURL(new Blob([png as BlobPart], { type: "image/png" }));
  });
}

/** Draws `document` on the server a moment after it stops changing. A newer
 * document cancels the drawing of an older one. */
export function usePreview(document: unknown, display: Display | undefined, deviceId: number | undefined): Preview {
  const [preview, setPreview] = useState<Preview>({ problems: [], drawing: false, stale: false });
  const key = JSON.stringify([document, display, deviceId]);

  useEffect(() => {
    if (document === undefined || !display) return;
    const abort = new AbortController();
    const timer = window.setTimeout(async () => {
      setPreview((current) => ({ ...current, drawing: true }));
      try {
        const output = await client.previewScreen(
          { document: document as ScreenDocument, display, deviceId },
          { abortSignal: abort.signal },
        );
        const url = output.image ? await dataUrl(output.image) : undefined;
        if (abort.signal.aborted) return;
        setPreview((current) => ({
          image: url ?? current.image,
          sources: output.sources,
          problems: output.problems,
          drawing: false,
          stale: !url,
        }));
      } catch (error) {
        if (abort.signal.aborted) return;
        // The server checks the document against the model before drawing; its
        // field paths name the request's `document` member.
        const problems =
          error instanceof ValidationException && error.fieldList?.length
            ? error.fieldList.map((field) => ({
                // The path is shown beside the message, so its own copy of it goes.
                message: field.message.replace(/^Value at '[^']*' failed to satisfy constraint: /, ""),
                path: field.path.replace(/^\/document/, ""),
              }))
            : [];
        setPreview((current) => ({
          ...current,
          problems,
          drawing: false,
          stale: true,
          failure: problems.length > 0 ? undefined : messageOf(error),
        }));
      }
    }, WAIT);
    return () => {
      window.clearTimeout(timer);
      abort.abort();
      setPreview((current) => (current.drawing ? { ...current, drawing: false } : current));
    };
    // `key` carries every input; the objects themselves are new on every render.
  }, [key]);

  return preview;
}
