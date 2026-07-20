import { readFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDir, '..', '..');
const rootManifest = JSON.parse(
  await readFile(path.join(repositoryRoot, 'package.json'), 'utf8')
);
const releaseCargoManifests = [
  'crates/codegen/xai-grok-pager-bin/Cargo.toml',
  'crates/codegen/xai-grok-pager/Cargo.toml',
  'crates/codegen/xai-grok-shell/Cargo.toml',
  'crates/codegen/xai-grok-version/Cargo.toml'
];

for (const relativeManifestPath of releaseCargoManifests) {
  const cargoManifest = await readFile(
    path.join(repositoryRoot, ...relativeManifestPath.split('/')),
    'utf8'
  );
  const cargoVersion = cargoManifest.match(
    /^version\s*=\s*"([^"]+)"\s*$/m
  )?.[1];

  if (!cargoVersion) {
    throw new Error(`Unable to read the version from ${relativeManifestPath}.`);
  }

  if (rootManifest.version !== cargoVersion) {
    throw new Error(
      `Version mismatch: package.json=${rootManifest.version}, ${relativeManifestPath}=${cargoVersion}`
    );
  }
}

const expectedNativePackages = [
  'chutes-build-darwin-arm64',
  'chutes-build-darwin-x64',
  'chutes-build-linux-arm64-gnu',
  'chutes-build-linux-x64-gnu',
  'chutes-build-win32-arm64',
  'chutes-build-win32-x64'
];

for (const packageName of expectedNativePackages) {
  const dependencyVersion = rootManifest.optionalDependencies?.[packageName];
  if (dependencyVersion !== rootManifest.version) {
    throw new Error(
      `Version mismatch: ${packageName}=${dependencyVersion ?? 'missing'}, expected=${rootManifest.version}`
    );
  }
}

const unexpectedNativePackages = Object.keys(
  rootManifest.optionalDependencies ?? {}
).filter((packageName) => !expectedNativePackages.includes(packageName));

if (unexpectedNativePackages.length > 0) {
  throw new Error(
    `Unexpected optional dependencies: ${unexpectedNativePackages.join(', ')}`
  );
}

process.stdout.write(
  `Release manifests are aligned at ${rootManifest.version} for ${releaseCargoManifests.length} Cargo packages and ${expectedNativePackages.length} native npm packages.\n`
);
