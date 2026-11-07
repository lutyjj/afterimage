import { useQuery } from "@tanstack/react-query";
import { Form, useNavigation } from "react-router";

import { accountQuery } from "../app/client";
import { Row, Rows, Section } from "../app/Section";
import { setTheme, THEMES, useTheme, type Theme } from "../app/theme";
import styles from "./Settings.module.css";

const THEME_LABELS: Record<Theme, string> = { auto: "Auto", light: "Light", dark: "Dark" };

/** How the web UI looks on this browser, and who is signed in. */
export function Preferences() {
  const theme = useTheme();
  const account = useQuery(accountQuery).data;
  const signingOut = useNavigation().formAction === "/sign-out";

  return (
    <>
      <Section title="Appearance">
        <Rows>
          <Row label="Theme" description="Auto follows this device's setting. Kept in this browser.">
            <div className={styles.segmented} role="radiogroup" aria-label="Theme">
              {THEMES.map((option) => (
                <button
                  key={option}
                  type="button"
                  role="radio"
                  aria-checked={theme === option}
                  onClick={() => setTheme(option)}
                >
                  {THEME_LABELS[option]}
                </button>
              ))}
            </div>
          </Row>
        </Rows>
      </Section>
      <Section title="Account">
        <Rows>
          <Row
            label={account?.name}
            description={
              account?.username
                ? `Signs in with the username ${account.username}.`
                : "Signs in through single sign-on."
            }
          >
            {account?.administrator && <span className={styles.tag}>Administrator</span>}
          </Row>
          <Row label="Sign out" description="Ends this browser's session. Access tokens keep working.">
            <Form method="post" action="/sign-out">
              <button type="submit" className="button" disabled={signingOut}>
                Sign out
              </button>
            </Form>
          </Row>
        </Rows>
      </Section>
    </>
  );
}
