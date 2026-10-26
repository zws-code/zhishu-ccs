#!/usr/bin/env node
// Generates the website download manifest (manifest.json) from a directory of
// downloaded release assets. Deploy the output as zhishu-proxy/client-release.json.
// Keep its schema compatible with frontend/src/utils/clientRelease.ts in zhishu-proxy.
//
// Usage: node scripts/generate-download-manifest.mjs <assets-dir> <tag> <base-url> [output] [pub-date]

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { productionUrl } from "./release-config.mjs";

const [assetsDir, tag, baseUrl, output = "manifest.json", pubDateArg] =
  process.argv.slice(2);

if (!assetsDir || !tag || !baseUrl) {
  console.error(
    "Usage: node scripts/generate-download-manifest.mjs <assets-dir> <tag> <base-url> [output] [pub-date]",
  );
  process.exit(1);
}

// Prefer the release's real publishedAt (passed by CI) over generation time.
const pubDate = pubDateArg ? new Date(pubDateArg) : new Date();
if (Number.isNaN(pubDate.getTime())) {
  console.error(`Invalid pub-date: ${pubDateArg}`);
  process.exit(1);
}

// Longer suffixes must come before their shorter counterparts
// (e.g. -Windows-arm64.msi before -Windows.msi).
const RULES = [
  { suffix: "-macOS.dmg", platform: "macos", kind: "dmg", arch: "universal" },
  { suffix: "-macOS.zip", platform: "macos", kind: "zip", arch: "universal" },
  {
    suffix: "-Windows-arm64-Portable.zip",
    platform: "windows",
    kind: "portable",
    arch: "arm64",
  },
  {
    suffix: "-Windows-Portable.zip",
    platform: "windows",
    kind: "portable",
    arch: "x64",
  },
  {
    suffix: "-Windows-arm64.msi",
    platform: "windows",
    kind: "msi",
    arch: "arm64",
  },
  { suffix: "-Windows.msi", platform: "windows", kind: "msi", arch: "x64" },
  {
    suffix: "-Linux-arm64.AppImage",
    platform: "linux",
    kind: "appimage",
    arch: "arm64",
  },
  {
    suffix: "-Linux-x86_64.AppImage",
    platform: "linux",
    kind: "appimage",
    arch: "x64",
  },
  { suffix: "-Linux-arm64.deb", platform: "linux", kind: "deb", arch: "arm64" },
  { suffix: "-Linux-x86_64.deb", platform: "linux", kind: "deb", arch: "x64" },
  { suffix: "-Linux-arm64.rpm", platform: "linux", kind: "rpm", arch: "arm64" },
  { suffix: "-Linux-x86_64.rpm", platform: "linux", kind: "rpm", arch: "x64" },
];

if (!/^v?\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.-]+)?$/.test(tag))
  throw new Error("Invalid release tag");
const normalizedBase = productionUrl(baseUrl, "download base URL", {
  allowInsecureHttp: process.env.ZHISHU_DOWNLOAD_ALLOW_INSECURE_HTTP === "true",
}).replace(/\/+$/, "");
const files = [];

for (const name of readdirSync(assetsDir).sort()) {
  // Unmatched files (.sig, .tar.gz updater artifacts, latest.json) are
  // deliberately skipped — they are not user-facing downloads.
  const rule = RULES.find((entry) => name.endsWith(entry.suffix));
  if (!rule) continue;
  const path = join(assetsDir, name);
  const info = statSync(path);
  if (!info.isFile() || info.size <= 0)
    throw new Error(`Invalid installer: ${name}`);
  files.push({
    platform: rule.platform,
    kind: rule.kind,
    arch: rule.arch,
    name,
    size: info.size,
    sha256: createHash("sha256").update(readFileSync(path)).digest("hex"),
    url: `${normalizedBase}/${tag}/${encodeURIComponent(name)}`,
  });
}

if (files.length === 0) {
  console.error(`No release assets matched in ${assetsDir}`);
  process.exit(1);
}

const manifest = {
  version: tag.replace(/^v/, ""),
  tag,
  pubDate: pubDate.toISOString(),
  files,
  ...(process.env.ZHISHU_DOWNLOAD_ALLOW_INSECURE_HTTP === "true"
    ? { allowInsecureHttp: true }
    : {}),
};

writeFileSync(output, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Wrote ${output} with ${files.length} files for ${tag}`);
