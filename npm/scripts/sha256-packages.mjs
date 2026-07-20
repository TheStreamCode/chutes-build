import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

const mode = process.argv[2];
const directory = path.resolve(process.argv[3] ?? 'dist');

if (!['write', 'verify'].includes(mode)) {
  throw new Error('Usage: node sha256-packages.mjs <write|verify> [directory]');
}

async function sha256(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) {
    hash.update(chunk);
  }
  return hash.digest('hex');
}

const entries = (await readdir(directory)).sort();

if (mode === 'write') {
  const archives = entries.filter((entry) => entry.endsWith('.tgz'));
  if (archives.length === 0) {
    throw new Error(`No .tgz packages found in ${directory}.`);
  }
  for (const archive of archives) {
    const digest = await sha256(path.join(directory, archive));
    await writeFile(
      path.join(directory, `${archive}.sha256`),
      `${digest}  ${archive}\n`,
      'utf8'
    );
  }
  process.stdout.write(`Wrote ${archives.length} SHA-256 sidecar(s).\n`);
} else {
  const sidecars = entries.filter((entry) => entry.endsWith('.tgz.sha256'));
  if (sidecars.length === 0) {
    throw new Error(`No package checksum sidecars found in ${directory}.`);
  }
  for (const sidecar of sidecars) {
    const line = (await readFile(path.join(directory, sidecar), 'utf8')).trim();
    const match = /^([a-f0-9]{64}) {2}([^/\\]+\.tgz)$/.exec(line);
    if (!match || `${match[2]}.sha256` !== sidecar) {
      throw new Error(`Invalid checksum sidecar: ${sidecar}.`);
    }
    const actual = await sha256(path.join(directory, match[2]));
    if (actual !== match[1]) {
      throw new Error(`Checksum mismatch for ${match[2]}.`);
    }
  }
  process.stdout.write(`Verified ${sidecars.length} package checksum(s).\n`);
}
