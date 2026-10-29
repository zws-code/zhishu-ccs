#!/usr/bin/env node
// COS release configuration and signed payload preparation. No cloud credentials.
import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";
import {
  artifactNames,
  collectUpdaterPlatforms,
  hashFile,
  targets,
  versionNumber,
} from "./release-artifacts.mjs";
import { productionUrl, readReleaseConfig } from "./release-config.mjs";

export function cosConfig(env = process.env) {
  const bucket = env.COS_BUCKET;
  const region = env.COS_REGION;
  const prefix = env.COS_PREFIX || "ccs/stable";
  if (!/^[a-z0-9][a-z0-9-]*-\d+$/.test(bucket || ""))
    throw new Error("COS_BUCKET 必须是包含 APPID 后缀的桶名");
  if (!/^[a-z]+-[a-z]+(?:-[a-z]+)*-?\d*$/.test(region || ""))
    throw new Error("请配置 COS_REGION，例如 ap-guangzhou");
  if (!/^[A-Za-z0-9_-]+(?:\/[A-Za-z0-9_-]+)*$/.test(prefix))
    throw new Error("COS_PREFIX 必须是桶内相对目录");
  const base = productionUrl(
    env.COS_PUBLIC_BASE_URL,
    "COS_PUBLIC_BASE_URL",
  ).replace(/\/+$/, "");
  if (new URL(base).pathname !== `/${prefix}`)
    throw new Error("COS_PUBLIC_BASE_URL 的路径必须与 COS_PREFIX 一致");
  if (env.ZHISHU_UPDATE_URL !== `${base}/latest.json`)
    throw new Error(
      "ZHISHU_UPDATE_URL 必须等于 COS_PUBLIC_BASE_URL + /latest.json",
    );
  return {
    bucket,
    region,
    prefix,
    downloadBaseUrl: base,
    allowInsecureHttp: false,
  };
}

export function checkVersion(root, tag) {
  const version = versionNumber(tag);
  if (tag !== `v${version}`) throw new Error("发布 tag 必须是 vX.Y.Z");
  const pkg = JSON.parse(readFileSync(join(root, "package.json")));
  const tauri = JSON.parse(
    readFileSync(join(root, "src-tauri/tauri.conf.json")),
  );
  const cargo = readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8").match(
    /^version\s*=\s*"([^"]+)"/m,
  )?.[1];
  if ([pkg.version, tauri.version, cargo].some((v) => v !== version))
    throw new Error(
      "tag 与 package.json、tauri.conf.json、Cargo.toml 版本不一致；先提交版本修改",
    );
  if (!existsSync(join(root, `src/whats-new/${version}.json`)))
    throw new Error("缺少本版本的应用内更新说明");
  return version;
}

export function prepareCosPayload(assets, tag, out, env = process.env) {
  const publish = cosConfig(env);
  const config = readReleaseConfig(new URL("../../", import.meta.url), env);
  const version = versionNumber(tag);
  if (tag !== `v${version}`) throw new Error("发布 tag 必须是 vX.Y.Z");
  const source = JSON.parse(readFileSync(join(assets, "latest.json")));
  if (
    source.version !== version ||
    !source.pub_date ||
    Number.isNaN(Date.parse(source.pub_date))
  )
    throw new Error("更新清单版本或发布时间无效");
  const ids = Object.keys(targets);
  // Validates every required installer and the actual minisign signatures with our public key.
  const platforms = collectUpdaterPlatforms(
    assets,
    ids,
    version,
    config,
    publish,
  );
  const names = new Set();
  for (const id of ids) {
    for (const [ext, name] of Object.entries(artifactNames(id, version))) {
      names.add(name);
      if (!["dmg", "zip"].includes(ext)) names.add(`${name}.sig`);
    }
  }
  // Preserve the existing upstream Windows portable downloads when present.
  for (const arch of ["", "-arm64"]) {
    const name = `Zhishu-Ccs-${tag}-Windows${arch}-Portable.zip`;
    if (existsSync(join(assets, name))) names.add(name);
  }
  const allowed = new Set([...names, "latest.json", "client-release.json"]);
  for (const name of readdirSync(assets)) {
    if (!allowed.has(name) || !lstatSync(join(assets, name)).isFile())
      throw new Error(`发布目录含非预期文件：${name}`);
  }
  mkdirSync(out, { recursive: true });
  if (readdirSync(out).length) throw new Error("COS 输出目录必须为空");
  for (const name of names) copyFileSync(join(assets, name), join(out, name));
  const generated = spawnSync(
    process.execPath,
    [
      fileURLToPath(
        new URL("../generate-download-manifest.mjs", import.meta.url),
      ),
      out,
      tag,
      publish.downloadBaseUrl,
      join(out, "client-release.json"),
      source.pub_date,
    ],
    {
      encoding: "utf8",
      env: { ...env, ZHISHU_DOWNLOAD_ALLOW_INSECURE_HTTP: "false" },
    },
  );
  if (generated.status !== 0)
    throw new Error(generated.stderr || "下载清单生成失败");
  writeFileSync(
    join(out, "latest.json"),
    JSON.stringify({ ...source, platforms }, null, 2) + "\n",
  );
  const files = readdirSync(out)
    .sort()
    .map((name) => ({
      name,
      size: lstatSync(join(out, name)).size,
      sha256: hashFile(join(out, name)),
    }));
  const receipt = {
    version,
    tag,
    downloadBaseUrl: publish.downloadBaseUrl,
    targets: ids,
    files,
  };
  writeFileSync(
    join(out, "release.json"),
    JSON.stringify(receipt, null, 2) + "\n",
  );
  return receipt;
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  try {
    const [command, ...args] = process.argv.slice(2);
    if (command === "check") {
      cosConfig();
      readReleaseConfig(new URL("../../", import.meta.url));
      if (args[0])
        checkVersion(fileURLToPath(new URL("../../", import.meta.url)), args[0]);
      console.log("COS 发布配置检查通过；未连接腾讯云。");
    } else if (command === "prepare" && args.length === 3) {
      const receipt = prepareCosPayload(...args);
      console.log(
        `已验证 ${receipt.tag} 的签名，准备 ${receipt.files.length} 个公开文件。`,
      );
    } else
      throw new Error(
        "用法：node scripts/zhishu/cos-release.mjs check [tag] | prepare <assets> <tag> <output>",
      );
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
