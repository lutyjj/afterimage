/** Where a person without a session signs in: the sign-in page, told why the
 * provider's sign-in failed when the server sent them back with a reason. */
export function signInPath(url: URL): string {
  const reason = url.searchParams.get("signInError");
  return reason ? `/sign-in?${new URLSearchParams({ signInError: reason }).toString()}` : "/sign-in";
}
