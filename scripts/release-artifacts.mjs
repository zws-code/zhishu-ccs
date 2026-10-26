import { createHash, createPublicKey, verify } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
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
    bundles: "msi",
    platform: "windows",
    arch: "x64",
  },
  "windows-arm64": {
    os: "win32",
    triple: "aarch64-pc-windows-msvc",
    bundles: "msi",
    platform: "windows",
    arch: "arm64",
  },
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
export function artifactNames(id, version) {
  const target = targets[id];
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
        ? ["msi"]
        : ["AppImage", "deb", "rpm"];
  return Object.fromEntries(
    extensions.map((ext) => [ext, `${prefix}-${suffix}.${ext}`]),
  );
}
export function collectUpdaterPlatforms(dir, ids, version, config, publish) {
  const platforms = {};
  for (const id of ids) {
    const target = targets[id];
    for (const [ext, name] of Object.entries(artifactNames(id, version))) {
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
export function hashFile(path) {
  // Files are bounded by the installer size; Tauri's verifier also reads the payload.
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}
export function configFingerprint(config) {
  return createHash("sha256").update(JSON.stringify(config)).digest("hex");
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
export function walkFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isSymbolicLink()) return []; // .app contains framework symlinks; never follow these as bundles.
    return entry.isDirectory() ? walkFiles(path) : [path];
  });
}
export function oneFile(dir, predicate, label) {
  const files = walkFiles(dir).filter(predicate);
  if (files.length !== 1)
    throw new Error(`${label} 应有且只有一个，实际 ${files.length} 个：${dir}`);
  return files[0];
}
export function saveReceipt(dir, id, version, config) {
  const target = targets[id];
  if (!target) throw new Error(`未知目标：${id}`);
  const files = readdirSync(dir)
    .filter((name) => name !== "build.json")
    .sort()
    .map((name) => {
      const path = join(dir, name);
      if (!lstatSync(path).isFile() || !/^[A-Za-z0-9._-]+$/.test(name))
        throw new Error(`非法发布文件：${name}`);
      return { name, size: statSync(path).size, sha256: hashFile(path) };
    });
  const receipt = {
    version: versionNumber(version),
    target: id,
    configFingerprint: configFingerprint(config),
    builtAt: new Date().toISOString(),
    files,
  };
  writeFileSync(
    join(dir, "build.json"),
    JSON.stringify(receipt, null, 2) + "\n",
  );
  return receipt;
}
export function validateReceipt(dir, version, config) {
  const receipt = JSON.parse(readFileSync(join(dir, "build.json"), "utf8"));
  if (
    receipt.version !== versionNumber(version) ||
    !targets[receipt.target] ||
    receipt.configFingerprint !== configFingerprint(config) ||
    !Array.isArray(receipt.files) ||
    !receipt.files.length
  )
    throw new Error(`安装包版本/平台地址/签名公钥不一致：${dir}`);
  const names = new Set();
  for (const file of receipt.files) {
    if (
      typeof file.name !== "string" ||
      !/^[A-Za-z0-9._-]+$/.test(file.name) ||
      names.has(file.name)
    )
      throw new Error("发布文件名不合法或重复");
    names.add(file.name);
    const path = join(dir, file.name);
    if (
      !lstatSync(path).isFile() ||
      statSync(path).size !== file.size ||
      hashFile(path) !== file.sha256
    )
      throw new Error(`安装包损坏：${path}`);
  }
  if (
    readdirSync(dir).some((name) => name !== "build.json" && !names.has(name))
  )
    throw new Error(`构建目录含清单外文件：${dir}`);
  return receipt;
}
export function assembleRelease(
  dirs,
  out,
  version,
  config,
  publish,
  notes = "",
) {
  version = versionNumber(version);
  const tag = `v${version}`;
  mkdirSync(out, { recursive: true });
  if (readdirSync(out).length) throw new Error(`发布目录必须为空：${out}`);
  const ids = new Set();
  for (const dir of dirs) {
    const receipt = validateReceipt(dir, version, config);
    if (ids.has(receipt.target))
      throw new Error(`重复构建目标：${receipt.target}`);
    ids.add(receipt.target);
    for (const file of receipt.files) {
      const to = join(out, file.name);
      if (existsSync(to)) throw new Error(`发布文件重名：${file.name}`);
      copyFileSync(join(dir, file.name), to);
    }
  }
  const pubDate = new Date().toISOString();
  const tool = fileURLToPath(
    new URL("./generate-download-manifest.mjs", import.meta.url),
  );
  const generated = spawnSync(
    process.execPath,
    [
      tool,
      out,
      tag,
      publish.downloadBaseUrl,
      join(out, "client-release.json"),
      pubDate,
    ],
    {
      encoding: "utf8",
      env: {
        ...process.env,
        ZHISHU_DOWNLOAD_ALLOW_INSECURE_HTTP: String(publish.allowInsecureHttp),
      },
    },
  );
  if (generated.status !== 0)
    throw new Error(generated.stderr || "生成下载清单失败");
  const website = JSON.parse(
    readFileSync(join(out, "client-release.json"), "utf8"),
  );
  const platforms = collectUpdaterPlatforms(out, ids, version, config, publish);
  if (!ids.size || website.files.length > 40)
    throw new Error("没有可发布安装包或超出下载清单限制");
  const latest = {
    version,
    notes: notes || `Zhishu CCS ${tag}`,
    pub_date: pubDate,
    platforms,
  };
  writeFileSync(
    join(out, "latest.json"),
    JSON.stringify(latest, null, 2) + "\n",
  );
  const files = readdirSync(out)
    .sort()
    .map((name) => ({
      name,
      size: statSync(join(out, name)).size,
      sha256: hashFile(join(out, name)),
    }));
  const metadata = { version, tag, targets: [...ids].sort(), files };
  writeFileSync(
    join(out, "release.json"),
    JSON.stringify(metadata, null, 2) + "\n",
  );
  writeFileSync(
    join(out, "SHA256SUMS"),
    [
      ...files.map((file) => `${file.sha256}  ${file.name}`),
      `${hashFile(join(out, "release.json"))}  release.json`,
    ].join("\n") + "\n",
  );
  return metadata;
}
