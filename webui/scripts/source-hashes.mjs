import { createHash } from 'node:crypto';
import { readdir, readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const webuiRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifestPath = path.join(webuiRoot, 'dist', 'source-hashes.txt');
const rootInputs = [
  'index.html',
  'package.json',
  'package-lock.json',
  'postcss.config.mjs',
  'scripts/source-hashes.mjs',
  'tailwind.config.ts',
  'tsconfig.app.json',
  'tsconfig.json',
  'tsconfig.node.json',
  'vite.config.ts',
];
const inputDirectories = ['public', 'src'];

async function collectFiles(relativeDirectory) {
  const absoluteDirectory = path.join(webuiRoot, relativeDirectory);
  const entries = await readdir(absoluteDirectory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const relativePath = path.posix.join(relativeDirectory, entry.name);
    if (entry.isDirectory()) {
      files.push(...await collectFiles(relativePath));
    } else if (entry.isFile()) {
      files.push(relativePath);
    }
  }

  return files;
}

async function buildManifest() {
  const files = [...rootInputs];
  for (const directory of inputDirectories) {
    files.push(...await collectFiles(directory));
  }
  files.sort();

  const lines = [];
  for (const relativePath of files) {
    const content = await readFile(path.join(webuiRoot, relativePath));
    const digest = createHash('sha256').update(content).digest('hex');
    lines.push(`${digest}  ${relativePath}`);
  }
  return `${lines.join('\n')}\n`;
}

const expected = await buildManifest();
if (process.argv.includes('--check')) {
  let actual = '';
  try {
    actual = await readFile(manifestPath, 'utf8');
  } catch {
    console.error('WebUI source hash manifest is missing; run npm run build in webui/.');
    process.exit(1);
  }
  if (actual !== expected) {
    console.error('WebUI dist is stale; run npm run build in webui/.');
    process.exit(1);
  }
  console.log('WebUI source hash manifest is current.');
} else {
  await mkdir(path.dirname(manifestPath), { recursive: true });
  await writeFile(manifestPath, expected, 'utf8');
  console.log(`Wrote ${path.relative(webuiRoot, manifestPath)}.`);
}
