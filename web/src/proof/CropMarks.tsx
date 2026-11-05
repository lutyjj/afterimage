import styles from "./CropMarks.module.css";

/** Printer's crop marks at the top-left and bottom-right corners of the
 * positioned element they sit in. */
export function CropMarks() {
  return (
    <>
      <span className={styles.mark} data-corner="top-left" aria-hidden="true" />
      <span className={styles.mark} data-corner="bottom-right" aria-hidden="true" />
    </>
  );
}
