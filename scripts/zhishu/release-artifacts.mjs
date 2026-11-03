import { createHash, createPublicKey, verify } from "node:crypto";
import { lstatSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { parseUpdatePublicKey } from "./release-config.mjs";

export const targets = {
  macos: {
    os: "darwin",
    triple: "universal-apple-darwin",
    bundles: "app",
    platform: "macos",
    arch: "universal",
  },
  "windows-x64": {
    os: "win32",
    triple: "x86_64-pc-windows-msvc",
    bundles: "nsis",
    platform: "windows",
    arch: "x64",
  },
  "windows-arm64": {
    os: "win32",
    triple: "aarch64-pc-windows-msvc",
    bundles: "nsis",
    platform: "windows",
    arch: "arm64",
  },
};
// Only for validating immutable payloads created before the EXE-only policy.
export const legacyTargets = {
  ...targets,
  "windows-x64": { ...targets["windows-x64"], bundles: "msi" },
  "windows-arm64": { ...targets["windows-arm64"], bundles: "msi" },
  "linux-x64": {
    os: "linux",
    triple: "x86_64-unknown-linux-gnu",
    bundles: "appimage,deb,rpm",
    platform: "linux",
    arch: "x64",
  },
  "linux-arm64": {
    os: "linux",
    triple: "aarch64-unknown-linux-gnu",
    bundles: "appimage,deb,rpm",
    platform: "linux",
    arch: "arm64",
  },
};

export function versionNumber(value) {
  const v = String(value).replace(/^v/, "");
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(v))
    throw new Error("正式版本必须是 x.y.z，例如 4.0.20");
  return v;
}
export function artifactNames(id, version, definitions = targets) {
  const target = definitions[id];
  if (!target) throw new Error(`未知目标：${id}`);
  const prefix = `Zhishu-Ccs-v${versionNumber(version)}`;
  const suffix =
    id === "macos"
      ? "macOS"
      : target.platform === "windows"
        ? `Windows${target.arch === "arm64" ? "-arm64" : ""}`
        : `Linux-${target.arch === "x64" ? "x86_64" : "arm64"}`;
  const extensions =
    id === "macos"
      ? ["dmg", "zip", "tar.gz"]
      : target.platform === "windows"
        ? [target.bundles === "nsis" ? "exe" : "msi"]
        : ["AppImage", "deb", "rpm"];
  return Object.fromEntries(
    extensions.map((ext) => [ext, `${prefix}-${suffix}.${ext}`]),
  );
}
export function collectUpdaterPlatforms(
  dir,
  ids,
  version,
  config,
  publish,
  definitions = targets,
) {
  const platforms = {};
  for (const id of ids) {
    const target = definitions[id];
    for (const [ext, name] of Object.entries(
      artifactNames(id, version, definitions),
    )) {
      const path = join(dir, name);
      if (!lstatSync(path).isFile() || statSync(path).size <= 0)
        throw new Error(`安装包无效：${path}`);
      if (ext === "dmg" || ext === "zip") continue;
      const signature = readFileSync(`${path}.sig`, "utf8").trim();
      verifySignature(path, signature, config.updatePublicKey);
      const key = `${target.platform}-${target.arch === "x64" ? "x86_64" : "aarch64"}`;
      const keys =
        id === "macos"
          ? ["darwin-aarch64", "darwin-x86_64"]
          : [ext === "deb" || ext === "rpm" ? `${key}-${ext}` : key];
      for (const key of keys) {
        if (platforms[key]) throw new Error(`重复更新目标：${key}`);
        platforms[key] = {
          signature,
          url: `${publish.downloadBaseUrl}/v${versionNumber(version)}/${encodeURIComponent(name)}`,
        };
      }
    }
  }
  return platforms;
}
// Recovery uses the original receipt's packaging policy, never today's build matrix.
export function collectReceiptUpdaterPlatforms(dir, receipt, config, publish) {
  const definitions =
    receipt.packaging === "nsis-v1"
      ? targets
      : receipt.packaging === undefined
        ? legacyTargets
        : null;
  if (
    !definitions ||
    !Array.isArray(receipt.targets) ||
    JSON.stringify([...receipt.targets].sort()) !==
      JSON.stringify(Object.keys(definitions).sort())
  )
    throw new Error("发布收据的打包策略或目标不一致");
  return collectUpdaterPlatforms(
    dir,
    receipt.targets,
    receipt.version,
    config,
    publish,
    definitions,
  );
}

export function hashFile(path) {
  // Files are bounded by the installer size; Tauri's verifier also reads the payload.
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}
export function verifySignature(path, signature, publicKey) {
  const pub = parseUpdatePublicKey(publicKey);
  const lines = Buffer.from(signature.trim(), "base64")
    .toString("utf8")
    .trim()
    .split(/\r?\n/);
  const sig = Buffer.from(lines[1] ?? "", "base64");
  const global = Buffer.from(lines[3] ?? "", "base64");
  if (
    sig.length !== 74 ||
    global.length !== 64 ||
    !lines[2]?.startsWith("trusted comment: ") ||
    !sig.subarray(2, 10).equals(pub.subarray(2, 10))
  )
    throw new Error(`签名格式或公钥不匹配：${path}`);
  const algorithm = sig.subarray(0, 2).toString();
  const key = createPublicKey({
    key: Buffer.concat([
      Buffer.from("302a300506032b6570032100", "hex"),
      pub.subarray(10),
    ]),
    format: "der",
    type: "spki",
  });
  const data = readFileSync(path);
  const message =
    algorithm === "ED" ? createHash("blake2b512").update(data).digest() : data;
  if (
    !["ED", "Ed"].includes(algorithm) ||
    !verify(null, message, key, sig.subarray(10)) ||
    !verify(
      null,
      Buffer.concat([sig.subarray(10), Buffer.from(lines[2].slice(17))]),
      key,
      global,
    )
  )
    throw new Error(`更新签名验证失败：${path}`);
}
