import { spawnSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const binaryArgument = process.argv[2];
if (!binaryArgument) {
  throw new Error('Usage: node verify-native-binary.mjs <binary>');
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDir, '..', '..');
const manifest = JSON.parse(
  await readFile(path.join(repositoryRoot, 'package.json'), 'utf8')
);
const binary = path.resolve(binaryArgument);
const result = spawnSync(binary, ['--version'], {
  encoding: 'utf8',
  timeout: 15_000,
  windowsHide: true
});

if (result.error) {
  throw result.error;
}
if (result.status !== 0) {
  throw new Error(
    `Native binary exited with ${result.status}: ${result.stderr || result.stdout}`
  );
}

const output = `${result.stdout}\n${result.stderr}`.trim();
if (!output.includes(manifest.version)) {
  throw new Error(
    `Native binary version output ${JSON.stringify(output)} does not contain ${manifest.version}.`
  );
}

process.stdout.write(`${output}\n`);
