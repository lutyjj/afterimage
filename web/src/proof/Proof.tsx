import { useEffect, useRef, useState, type ReactNode } from "react";

import { CropMarks } from "./CropMarks";
import styles from "./Proof.module.css";

export interface ProofProps {
  /** The image a display shows, or absent before it has one. */
  src?: string | undefined;
  alt: string;
  /** The display's size in pixels, for the frame before the image loads; absent
   * until its model is known, when the frame keeps no size of its own. */
  width?: number | undefined;
  height?: number | undefined;
  /** The palette's colors, darkest gray first; absent for full color. */
  colors?: string[] | undefined;
  paletteName?: string | undefined;
  /** What the frame says when there is no image. */
  empty?: string;
  /** Drawn over the image, such as a layout grid. */
  overlay?: ReactNode;
  /** The image is being replaced. */
  busy?: boolean;
}

/** A display's image framed as a proof: crop marks at two corners, the image at
 * its true pixels when there is room, and beneath it the palette's step wedge
 * with the size and scale noted in blue. */
export function Proof({
  src,
  alt,
  width,
  height,
  colors,
  paletteName,
  empty = "No image yet",
  overlay,
  busy = false,
}: ProofProps) {
  const image = useRef<HTMLImageElement>(null);
  const [natural, setNatural] = useState({ width: width ?? 0, height: height ?? 0 });
  const [scale, setScale] = useState(1);

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

  return (
    <figure className={styles.proof}>
      <div
        className={styles.plate}
        style={sized ? { maxWidth: `${shownWidth}px`, aspectRatio: `${shownWidth} / ${shownHeight}` } : undefined}
        data-unsized={!sized || undefined}
        data-busy={busy || undefined}
      >
        <CropMarks />
        {src ? (
          <img
            ref={image}
            className={styles.image}
            src={src}
            alt={alt}
            width={sized ? shownWidth : undefined}
            height={sized ? shownHeight : undefined}
            data-upscaled={scale >= 1 || undefined}
            draggable={false}
          />
        ) : (
          <div className={styles.empty}>
            <span className="annotation">{empty}</span>
          </div>
        )}
        {overlay}
      </div>
      <figcaption className={styles.caption}>
        <Wedge colors={colors} />
        <span className="annotation">
          {paletteName && <span className={styles.palette}>{paletteName}</span>}
          {sized && `${shownWidth} × ${shownHeight}`}
          {src && <span className={styles.scale}>{scaleLabel(scale)}</span>}
        </span>
      </figcaption>
    </figure>
  );
}

function scaleLabel(scale: number) {
  if (Math.abs(scale - 1) < 0.01) return "1:1";
  return `at ${Math.round(scale * 100)}%`;
}

const MAX_STEPS = 16;

/** The palette's levels as a row of patches; a palette of thousands of colors is
 * a continuous band. */
function Wedge({ colors }: { colors?: string[] | undefined }) {
  if (!colors || colors.length === 0) {
    return <span className={styles.band} aria-hidden="true" />;
  }
  const stride = Math.max(1, Math.ceil(colors.length / MAX_STEPS));
  const steps = colors.filter((_, index) => index % stride === 0 || index === colors.length - 1);
  return (
    <span className={styles.wedge} aria-hidden="true">
      {steps.map((color, index) => (
        <span key={`${color}-${index}`} className={styles.step} style={{ background: color }} />
      ))}
    </span>
  );
}
