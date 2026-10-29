import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const target = resolve(root, 'src-tauri', 'PresentMon-2.5.1-x64.exe');
const url = 'https://github.com/GameTechDev/PresentMon/releases/download/v2.5.1/PresentMon-2.5.1-x64.exe';
const expectedSha256 = '9bec3083069f58f911e6a512f4806db51a27bd096103087bc1d05ef54c80a191';

try {
  const existing = await readFile(target);
  const actual = createHash('sha256').update(existing).digest('hex');
  if (actual === expectedSha256) {
    console.log('Verified bundled PresentMon 2.5.1.');
    process.exit(0);
  }
} catch {
  // Download on first build.
}

console.log('Downloading official PresentMon 2.5.1 release…');
const response = await fetch(url, { redirect: 'follow' });
if (!response.ok) throw new Error(`PresentMon download failed: HTTP ${response.status}`);
const binary = Buffer.from(await response.arrayBuffer());
const actualSha256 = createHash('sha256').update(binary).digest('hex');
if (actualSha256 !== expectedSha256) {
  throw new Error(`PresentMon SHA-256 mismatch (got ${actualSha256}); refusing to bundle it.`);
}
await mkdir(dirname(target), { recursive: true });
await writeFile(target, binary, { mode: 0o755 });
console.log(`Saved and verified ${target}`);
