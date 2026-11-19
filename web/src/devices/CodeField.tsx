import { Input } from "@base-ui/react/input";
import { classes } from "linen";
import { useState } from "react";

/** The characters a claim code uses: no 0, 1, I or O, which read alike. */
const ALPHABET = /[2-9A-HJ-NP-Z]/;
export const CODE_LENGTH = 6;

function cleanCode(text: string): string {
  return [...text.toUpperCase()]
    .filter((character) => ALPHABET.test(character))
    .join("")
    .slice(0, CODE_LENGTH);
}

export interface CodeFieldProps {
  value: string;
  onChange: (value: string) => void;
  invalid?: boolean;
  autoFocus?: boolean;
}

/** Six cells for the code a display shows. One real input carries the value, so
 * typing, pasting and assistive technology work as with any text field; the cells
 * draw it. It stands in a linen Field, which names it and says why it was
 * refused. */
export function CodeField({ value, onChange, invalid = false, autoFocus }: CodeFieldProps) {
  const [focused, setFocused] = useState(false);
  const active = Math.min(value.length, CODE_LENGTH - 1);
  return (
    <div className="relative">
      <div className="grid grid-cols-6 gap-2" aria-hidden="true">
        {Array.from({ length: CODE_LENGTH }, (_, index) => {
          const isActive = focused && index === active;
          const filled = index < value.length;
          return (
            <span
              key={index}
              className={classes(
                "grid aspect-[5/6] place-items-center rounded-button border bg-surface text-xl font-semibold tabular-nums",
                invalid
                  ? "border-danger"
                  : isActive
                    ? "border-accent shadow-[0_0_0_3px_color-mix(in_oklch,var(--color-accent)_18%,transparent)]"
                    : filled
                      ? "border-[color-mix(in_oklch,var(--color-accent)_40%,var(--color-border-strong))]"
                      : "border-border-strong",
              )}
            >
              {value[index] ?? ""}
            </span>
          );
        })}
      </div>
      <Input
        className="absolute inset-0 w-full cursor-text opacity-0"
        value={value}
        onChange={(event) => onChange(cleanCode(event.target.value))}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        autoComplete="off"
        autoCapitalize="characters"
        autoCorrect="off"
        spellCheck={false}
        inputMode="text"
        maxLength={CODE_LENGTH + 4}
        autoFocus={autoFocus}
      />
    </div>
  );
}
