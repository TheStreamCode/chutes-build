import { chmod, copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const TARGETS = Object.freeze({
  'darwin-arm64': {
    packageName: 'chutes-build-darwin-arm64',
    os: 'darwin',
    cpu: 'arm64',
    executable: 'chutes-build'
  },
  'darwin-x64': {
    packageName: 'chutes-build-darwin-x64',
    os: 'darwin',
    cpu: 'x64',
    executable: 'chutes-build'
  },
  'linux-arm64-gnu': {
    packageName: 'chutes-build-linux-arm64-gnu',
    os: 'linux',
    cpu: 'arm64',
    executable: 'chutes-build'
  },
  'linux-x64-gnu': {
    packageName: 'chutes-build-linux-x64-gnu',
    os: 'linux',
    cpu: 'x64',
    executable: 'chutes-build'
  },
  'win32-arm64': {
    packageName: 'chutes-build-win32-arm64',
    os: 'win32',
    cpu: 'arm64',
    executable: 'chutes-build.exe'
  },
  'win32-x64': {
    packageName: 'chutes-build-win32-x64',
    os: 'win32',
    cpu: 'x64',
    executable: 'chutes-build.exe'
  }
});

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith('--') || !value) {
      throw new Error('Expected --target, --binary, and --out-dir values.');
    }
    values.set(name.slice(2), value);
  }
  return values;
}

const args = parseArgs(process.argv.slice(2));
const targetName = args.get('target');
const sourceBinary = args.get('binary');
const outputRoot = args.get('out-dir');
const target = TARGETS[targetName];

if (!target || !sourceBinary || !outputRoot) {
  throw new Error(
    `Usage: node npm/scripts/prepare-platform-package.mjs --target <${Object.keys(TARGETS).join('|')}> --binary <path> --out-dir <path>`
  );
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDir, '..', '..');
const rootManifest = JSON.parse(
  await readFile(path.join(repositoryRoot, 'package.json'), 'utf8')
);
const packageRoot = path.resolve(outputRoot, target.packageName);
const binaryDir = path.join(packageRoot, 'bin');
const destinationBinary = path.join(binaryDir, target.executable);

await mkdir(path.resolve(outputRoot), { recursive: true });
await mkdir(packageRoot, { recursive: false });
await mkdir(binaryDir, { recursive: false });
await copyFile(path.resolve(sourceBinary), destinationBinary);
if (target.os !== 'win32') {
  await chmod(destinationBinary, 0o755);
}

const manifest = {
  name: target.packageName,
  version: rootManifest.version,
  description: `${rootManifest.description} (${targetName} binary)`,
  license: rootManifest.license,
  author: rootManifest.author,
  homepage: rootManifest.homepage,
  repository: rootManifest.repository,
  bugs: rootManifest.bugs,
  os: [target.os],
  cpu: [target.cpu],
  files: ['bin', 'README.md', 'LICENSE', 'NOTICE'],
  engines: rootManifest.engines
};
if (target.os === 'linux') {
  manifest.libc = ['glibc'];
}

const readme = `# ${target.packageName}\n\n` +
  `Prebuilt ${targetName} binary for [Chutes Build](https://www.npmjs.com/package/chutes-build). ` +
  'This package is installed automatically; install `chutes-build` instead.\n';

await Promise.all([
  writeFile(
    path.join(packageRoot, 'package.json'),
    `${JSON.stringify(manifest, null, 2)}\n`,
    'utf8'
  ),
  writeFile(path.join(packageRoot, 'README.md'), readme, 'utf8'),
  copyFile(path.join(repositoryRoot, 'LICENSE'), path.join(packageRoot, 'LICENSE')),
  copyFile(path.join(repositoryRoot, 'NOTICE'), path.join(packageRoot, 'NOTICE'))
]);

console.log(packageRoot);
