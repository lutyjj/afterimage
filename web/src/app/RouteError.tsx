import { buttonClass, Content, EmptyState, TopBar } from "linen";
import { isRouteErrorResponse, Link, useRouteError } from "react-router";

import { messageOf } from "./client";

/** A page that could not load, with the reason and a way back. A full page
 * load, since the app around it may be what failed. */
export function RouteError() {
  const error = useRouteError();
  const message = isRouteErrorResponse(error) ? `${error.status} ${error.statusText}` : messageOf(error);
  return (
    <Content>
      <EmptyState title="This page could not load" description={message}>
        <a href="/devices" className={buttonClass()}>
          Back to devices
        </a>
      </EmptyState>
    </Content>
  );
}

export function NotFoundPage() {
  return (
    <>
      <TopBar title="Not found" />
      <Content>
        <EmptyState
          title="Nothing lives at this address"
          description="The page may have moved, or the link may be wrong."
        >
          <Link to="/devices" className={buttonClass()}>
            Back to devices
          </Link>
        </EmptyState>
      </Content>
    </>
  );
}
