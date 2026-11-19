/** The data sources a problem names as a whole, by the path the server gives a
 * source whose request failed: `/sources/<name>`, the name escaped as a JSON
 * pointer segment. */
export function sourcesNamed(problems: readonly { path?: string | undefined }[]): Set<string> {
  const named = new Set<string>();
  for (const { path } of problems) {
    const segments = path?.split("/");
    if (segments?.length !== 3 || segments[0] !== "" || segments[1] !== "sources") continue;
    named.add(segments[2]!.replaceAll("~1", "/").replaceAll("~0", "~"));
  }
  return named;
}
