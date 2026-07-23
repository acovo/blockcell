export function authenticatedFileEndpoint(
  apiBase: string,
  path: string,
  agentId: string | undefined,
  inline: boolean,
): string {
  const query = new URLSearchParams({ path });
  if (agentId) query.set('agent', agentId);
  return `${apiBase}/v1/files/${inline ? 'serve' : 'download'}?${query.toString()}`;
}
