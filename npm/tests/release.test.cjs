'use strict';

const assert = require('node:assert/strict');
const { mkdtemp, readFile, rm, writeFile } = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const repositoryRoot = path.resolve(__dirname, '..', '..');
const verifyRelease = path.join(
  repositoryRoot,
  'npm',
  'scripts',
  'verify-release.mjs'
);
const preparePlatformPackage = path.join(
  repositoryRoot,
  'npm',
  'scripts',
  'prepare-platform-package.mjs'
);
const checksumPackages = path.join(
  repositoryRoot,
  'npm',
  'scripts',
  'sha256-packages.mjs'
);

test('release manifests use one version', () => {
  const result = spawnSync(process.execPath, [verifyRelease], {
    cwd: repositoryRoot,
    encoding: 'utf8'
  });

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /4 Cargo packages and 6 native npm packages/);
});

test('platform package carries public project metadata', async (context) => {
  const temporaryRoot = await mkdtemp(
    path.join(os.tmpdir(), 'chutes-build-package-test-')
  );
  context.after(() => rm(temporaryRoot, { recursive: true, force: true }));

  const fakeBinary = path.join(temporaryRoot, 'chutes-build.exe');
  const outputRoot = path.join(temporaryRoot, 'staging');
  await writeFile(fakeBinary, 'test binary fixture', 'utf8');

  const result = spawnSync(
    process.execPath,
    [
      preparePlatformPackage,
      '--target',
      'win32-x64',
      '--binary',
      fakeBinary,
      '--out-dir',
      outputRoot
    ],
    { cwd: repositoryRoot, encoding: 'utf8' }
  );

  assert.equal(result.status, 0, result.stderr);

  const manifest = JSON.parse(
    await readFile(
      path.join(outputRoot, 'chutes-build-win32-x64', 'package.json'),
      'utf8'
    )
  );
  assert.equal(manifest.name, 'chutes-build-win32-x64');
  assert.equal(manifest.repository.url, 'git+https://github.com/TheStreamCode/chutes-build.git');
  assert.equal(manifest.homepage, 'https://github.com/TheStreamCode/chutes-build#readme');
  assert.deepEqual(manifest.os, ['win32']);
  assert.deepEqual(manifest.cpu, ['x64']);
});

test('package checksums detect archive changes', async (context) => {
  const temporaryRoot = await mkdtemp(
    path.join(os.tmpdir(), 'chutes-build-checksum-test-')
  );
  context.after(() => rm(temporaryRoot, { recursive: true, force: true }));

  const archive = path.join(temporaryRoot, 'chutes-build-0.1.0.tgz');
  await writeFile(archive, 'package fixture', 'utf8');

  const writeResult = spawnSync(
    process.execPath,
    [checksumPackages, 'write', temporaryRoot],
    { cwd: repositoryRoot, encoding: 'utf8' }
  );
  assert.equal(writeResult.status, 0, writeResult.stderr);

  const verifyResult = spawnSync(
    process.execPath,
    [checksumPackages, 'verify', temporaryRoot],
    { cwd: repositoryRoot, encoding: 'utf8' }
  );
  assert.equal(verifyResult.status, 0, verifyResult.stderr);

  await writeFile(archive, 'tampered package fixture', 'utf8');
  const tamperedResult = spawnSync(
    process.execPath,
    [checksumPackages, 'verify', temporaryRoot],
    { cwd: repositoryRoot, encoding: 'utf8' }
  );
  assert.notEqual(tamperedResult.status, 0);
  assert.match(tamperedResult.stderr, /Checksum mismatch/);
});
