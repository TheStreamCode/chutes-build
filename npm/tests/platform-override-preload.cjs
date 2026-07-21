'use strict';

// Preload script (via `node --require`) that overrides process.platform/arch
// before the launcher module loads, so launcher tests can exercise a
// platform/arch combination other than the one actually running the test --
// without needing to refactor the intentionally-small launcher to accept
// injected values.
const platform = process.env.CHUTES_TEST_PLATFORM;
const arch = process.env.CHUTES_TEST_ARCH;

if (platform) {
  Object.defineProperty(process, 'platform', { value: platform, configurable: true });
}
if (arch) {
  Object.defineProperty(process, 'arch', { value: arch, configurable: true });
}
