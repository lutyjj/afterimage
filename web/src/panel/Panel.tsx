import { useEffect, useRef, useState, type ReactNode } from "react";

import { classes } from "linen";

import { Mark } from "../app/Wordmark";

export interface PanelProps {
  /** The image a display shows, or absent before it has one. */
  src?: string | undefined;
  alt: string;
  /** The display's size in pixels, for the frame before the image loads. */
  width?: number | undefined;
  height?: number | undefined;
  /** The palette's colours, darkest first; absent for full colour. */
  colors?: string[] | undefined;
  paletteName?: string | undefined;
  /** What the panel says when there is no image. */
  empty?: string;
  /** Drawn over the image, such as a layout grid. */
  overlay?: ReactNode;
  /** The image is being replaced. */
  busy?: boolean;
}

/** A display's image at its true pixels where there is room, with the palette,
 * size and scale beneath it. When the image changes, the one before lingers and
 * fades, as an e-paper screen keeps a trace of what it showed. The trace is the
 * previous `src` drawn again: a data URL is itself the image, and a URL is
 * served from the document's own list of images it has already decoded, which
 * a browser keeps for the page's life whatever the response's cache headers. */
export function Panel({
  src,
  alt,
  width,
  height,
  colors,
  paletteName,
  empty = "No image yet",
  overlay,
  busy = false,
}: PanelProps) {
  const image = useRef<HTMLImageElement>(null);
  const [natural, setNatural] = useState({ width: width ?? 0, height: height ?? 0 });
  const [scale, setScale] = useState(1);
  const [shown, setShown] = useState(src);
  const [ghost, setGhost] = useState<string>();
  if (src !== shown) {
    setGhost(shown);
    setShown(src);
  }

  useEffect(() => {
    const element = image.current;
    if (!element) return;
    const measure = () => {
      if (element.naturalWidth > 0) {
        setNatural({ width: element.naturalWidth, height: element.naturalHeight });
        setScale(element.clientWidth / element.naturalWidth);
      }
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    element.addEventListener("load", measure);
    return () => {
      observer.disconnect();
      element.removeEventListener("load", measure);
    };
  }, [src]);

  const shownWidth = src ? natural.width : width;
  const shownHeight = src ? natural.height : height;
  const sized = shownWidth !== undefined && shownHeight !== undefined && shownWidth > 0 && shownHeight > 0;

  const pixelated = scale >= 1 ? "[image-rendering:pixelated]" : "";

  return (
    <figure className="grid min-w-0 justify-items-start gap-2">
      <div
        className={classes(
          "relative w-full overflow-hidden rounded-input bg-epaper shadow-[0_0_0_1px_var(--color-border-strong)] [transition:opacity_var(--motion-state)]",
          !sized && "min-h-32",
          busy && "opacity-60",
        )}
        style={sized ? { maxWidth: `${shownWidth}px`, aspectRatio: `${shownWidth} / ${shownHeight}` } : undefined}
      >
        {src ? (
          <img
            ref={image}
            className={classes("h-auto w-full select-none", pixelated)}
            src={src}
            alt={alt}
            width={sized ? shownWidth : undefined}
            height={sized ? shownHeight : undefined}
            draggable={false}
          />
        ) : (
          <div className="absolute inset-0 grid place-content-center justify-items-center gap-3 bg-sunken text-sm text-text-muted">
            <Mark className="size-10 opacity-55" />
            {empty}
          </div>
        )}
        {src && ghost && (
          <img
            key={ghost}
            className={classes(
              "pointer-events-none absolute inset-0 h-auto w-full animate-afterimage mix-blend-multiply select-none motion-reduce:[animation-duration:1ms]",
              pixelated,
            )}
            src={ghost}
            alt=""
            aria-hidden="true"
            onAnimationEnd={() => setGhost(undefined)}
          />
        )}
        {overlay}
      </div>
      <figcaption className="flex flex-wrap items-center gap-x-2.5 gap-y-1 text-xs text-text-muted">
        <Swatches colors={colors} />
        {paletteName && <span>{paletteName}</span>}
        {sized && <span className="tabular-nums">{`${shownWidth} × ${shownHeight}`}</span>}
        {src && <span className="tabular-nums">{scaleLabel(scale)}</span>}
      </figcaption>
    </figure>
  );
}

function scaleLabel(scale: number) {
  if (Math.abs(scale - 1) < 0.01) return "1:1";
  return `at ${Math.round(scale * 100)}%`;
}

const MAX_STEPS = 16;

/** The palette's levels as a row of chips; thousands of colours are a band. */
function Swatches({ colors }: { colors?: string[] | undefined }) {
  if (!colors || colors.length === 0) {
    return (
      <span
        aria-hidden="true"
        className="inline-block h-2.5 w-12 rounded-[2px] bg-[linear-gradient(90deg,#000,#f00,#ff0,#0f0,#0ff,#00f,#f0f,#fff)] shadow-[inset_0_0_0_1px_color-mix(in_oklch,var(--color-text)_18%,transparent)]"
      />
    );
  }
  const stride = Math.max(1, Math.ceil(colors.length / MAX_STEPS));
  const steps = colors.filter((_, index) => index % stride === 0 || index === colors.length - 1);
  return (
    <span className="inline-flex gap-0.5" aria-hidden="true">
      {steps.map((color, index) => (
        <span
          key={`${color}-${index}`}
          className="size-2.5 rounded-[2px] shadow-[inset_0_0_0_1px_color-mix(in_oklch,var(--color-text)_18%,transparent)]"
          style={{ background: color }}
        />
      ))}
    </span>
  );
}
