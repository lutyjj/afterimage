import { ValidationException, type Display, type Problem, type ScreenDocument, type SourceResult } from "@afterimage/management-client";
import { useEffect, useRef, useState } from "react";

import { client, messageOf } from "../../app/client";

export interface Preview {
  /** An object URL of the last image drawn; kept while the next one draws. */
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

/** Draws `document` on the server a moment after it stops changing. A newer
 * document cancels the drawing of an older one. */
export function usePreview(document: unknown, display: Display | undefined, deviceId: number | undefined): Preview {
  const [preview, setPreview] = useState<Preview>({ problems: [], drawing: false, stale: false });
  const image = useRef<string | undefined>(undefined);
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
        const url = output.image ? URL.createObjectURL(new Blob([output.image as BlobPart], { type: "image/png" })) : undefined;
        if (url) {
          if (image.current) URL.revokeObjectURL(image.current);
          image.current = url;
        }
        setPreview({
          image: image.current,
          sources: output.sources,
          problems: output.problems,
          drawing: false,
          stale: !url,
        });
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
    };
    // `key` carries every input; the objects themselves are new on every render.
  }, [key]);

  useEffect(
    () => () => {
      if (image.current) URL.revokeObjectURL(image.current);
    },
    [],
  );

  return preview;
}
