export function applyPersonaContent(
  contents: Record<string, string>,
  fileName: string,
  content: string,
): Record<string, string> {
  return { ...contents, [fileName]: content };
}

export async function saveBeforeSwitch(
  save: () => Promise<boolean>,
  switchFile: () => void,
): Promise<boolean> {
  if (!(await save())) return false;
  switchFile();
  return true;
}
