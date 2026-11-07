import { useQuery } from "@tanstack/react-query";
import { Button, Card, CardHead, Chip, SegmentedControl, SettingRow } from "linen";
import { Form, useNavigation } from "react-router";

import { accountQuery } from "../app/client";
import { THEME_OPTIONS, useTheme } from "../app/theme";

/** How the web UI looks on this browser, and who is signed in. */
export function Preferences() {
  const { preference, setPreference } = useTheme();
  const account = useQuery(accountQuery).data;
  const signingOut = useNavigation().formAction === "/sign-out";

  return (
    <>
      <Card>
        <CardHead title="Appearance" />
        <SettingRow title="Theme" description="Auto follows this device's setting. Kept in this browser.">
          <SegmentedControl label="Theme" value={preference} options={THEME_OPTIONS} onChange={setPreference} />
        </SettingRow>
      </Card>
      <Card>
        <CardHead title="Account" />
        <SettingRow
          title={account?.name}
          description={
            account?.username ? `Signs in with the username ${account.username}.` : "Signs in through single sign-on."
          }
        >
          {account?.administrator && <Chip size="row">Administrator</Chip>}
        </SettingRow>
        <SettingRow title="Sign out" description="Ends this browser's session. Access tokens keep working.">
          <Form method="post" action="/sign-out">
            <Button type="submit" disabled={signingOut}>
              Sign out
            </Button>
          </Form>
        </SettingRow>
      </Card>
    </>
  );
}
