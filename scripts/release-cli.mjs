#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHash, randomUUID } from "node:crypto";
import { readReleaseConfig, productionUrl } from "./release-config.mjs";
import {
  artifactNames,
  assembleRelease,
  collectUpdaterPlatforms,
  configFingerprint,
  hashFile,
  oneFile,
  saveReceipt,
  targets,
  validateReceipt,
  versionNumber,
} from "./release-artifacts.mjs";

export const root = fileURLToPath(new URL("../", import.meta.url));
const json = (path) => JSON.parse(readFileSync(path, "utf8"));
const writeJson = (path, value) =>
  writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
const expand = (path) =>
  resolve(root, path.startsWith("~/") ? join(homedir(), path.slice(2)) : path);
const quote = (value) => "'" + String(value).replaceAll("'", "'\\''") + "'";
function run(
  cmd,
  args,
  { capture = false, env = process.env, input, cwd = root } = {},
) {
  const result = spawnSync(cmd, args, {
    cwd,
    env,
    input,
    encoding: "utf8",
    stdio:
      capture || input !== undefined ? ["pipe", "pipe", "pipe"] : "inherit",
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(
      `${basename(cmd)} 执行失败（${result.status}）${capture ? "" : ": " + (result.stderr ?? "")}`,
    );
  return result.stdout ?? "";
}
function pnpm(args, options) {
  const manager = process.env.npm_execpath;
  if (!manager) throw new Error("请使用 pnpm release:... 执行");
  return /\.(cjs|mjs|js)$/i.test(manager)
    ? run(process.execPath, [manager, ...args], options)
    : run(manager, args, options);
}
export function readPublishConfig(projectRoot = root) {
  const file = join(projectRoot, "release.publish.local.json");
  if (!existsSync(file))
    throw new Error(
      "先运行 pnpm release:init，或复制 release.publish.example.json 为 release.publish.local.json",
    );
  const value = json(file);
  const downloadBaseUrl = productionUrl(
    value.downloadBaseUrl,
    "downloadBaseUrl",
    { allowInsecureHttp: value.allowInsecureHttp === true },
  ).replace(/\/+$/, "");
  if (!/^[A-Za-z0-9_.-]+@[A-Za-z0-9.-]+$/.test(value.server))
    throw new Error("server 必须是 user@host");
  for (const key of ["remoteRoot", "proxyManifest"]) {
    if (
      typeof value[key] !== "string" ||
      !/^\/[A-Za-z0-9_./-]+$/.test(value[key]) ||
      value[key].includes("..") ||
      value[key] === "/" ||
      value[key].endsWith("/")
    )
      throw new Error(`非法远端路径：${key}`);
  }
  if (
    !Number.isInteger(value.sshPort) ||
    value.sshPort < 1 ||
    value.sshPort > 65535
  )
    throw new Error("非法 sshPort");
  if (
    typeof value.sshKey !== "string" ||
    !value.sshKey ||
    typeof value.signingKey !== "string" ||
    !value.signingKey
  )
    throw new Error("必须设置 sshKey 和 signingKey");
  return {
    ...value,
    downloadBaseUrl,
    allowInsecureHttp: value.allowInsecureHttp === true,
  };
}
function sshOptions(config) {
  if (!existsSync(expand(config.sshKey)))
    throw new Error(`SSH 私钥不存在：${expand(config.sshKey)}`);
  return [
    "-o",
    "BatchMode=yes",
    "-o",
    "ConnectTimeout=15",
    "-o",
    "StrictHostKeyChecking=yes",
    "-i",
    expand(config.sshKey),
  ];
}
function ssh(config, command, options = {}) {
  return run(
    "ssh",
    [
      ...sshOptions(config),
      "-p",
      String(config.sshPort),
      config.server,
      command,
    ],
    options,
  );
}
function scp(config, from, to, recursive = false) {
  return run("scp", [
    ...sshOptions(config),
    "-P",
    String(config.sshPort),
    ...(recursive ? ["-r"] : []),
    from,
    `${config.server}:${to}`,
  ]);
}
function production(config) {
  const result = readReleaseConfig(pathToFileURL(root));
  if (result.updateUrl !== `${config.downloadBaseUrl}/latest.json`)
    throw new Error(
      "release.config.local.json 的 updateUrl 必须等于 downloadBaseUrl + /latest.json，请重新运行 release:init",
    );
  if (result.updateAllowInsecureHttp !== config.allowInsecureHttp)
    throw new Error("下载服务和更新 HTTP 开关不一致");
  return result;
}
function signingEnvironment(config) {
  const key = expand(config.signingKey);
  if (!existsSync(key))
    throw new Error("缺少更新签名私钥，先执行 pnpm release:init");
  if (process.platform !== "win32" && statSync(key).mode & 0o077)
    throw new Error(`私钥权限过宽，请执行 chmod 600 ${key}`);
  return {
    ...process.env,
    ZHISHU_UNSIGNED_BUILD: "false",
    ZHISHU_EDITABLE_BUILD: "false",
    TAURI_SIGNING_PRIVATE_KEY: readFileSync(key, "utf8").trim(),
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD:
      process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "",
  };
}
function initialize() {
  const path = join(root, "release.publish.local.json");
  if (!existsSync(path)) {
    copyFileSync(join(root, "release.publish.example.json"), path);
    chmodSync(path, 0o600);
  }
  const config = readPublishConfig();
  const key = expand(config.signingKey);
  mkdirSync(dirname(key), { recursive: true, mode: 0o700 });
  if (!existsSync(key)) {
    // Tauri prints signing material. Capture and discard it; only the .pub file is used below.
    pnpm(
      [
        "tauri",
        "signer",
        "generate",
        "--ci",
        "-p",
        process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "",
        "-w",
        key,
      ],
      { capture: true },
    );
    chmodSync(key, 0o600);
  }
  if (!existsSync(`${key}.pub`))
    throw new Error(`缺少公钥 ${key}.pub；不会自动替换已有私钥`);
  const publicKey = readFileSync(`${key}.pub`, "utf8").trim();
  const releasePath = join(root, "release.config.local.json");
  const previous = existsSync(releasePath) ? json(releasePath) : {};
  if (previous.updatePublicKey && previous.updatePublicKey !== publicKey)
    throw new Error("已有公钥与当前私钥的 .pub 不同；拒绝自动更换更新身份");
  const platform = previous.platformApiUrl || "http://124.223.73.2:8080";
  const updated = {
    ...previous,
    platformApiUrl: platform,
    platformWebUrl: previous.platformWebUrl || platform,
    platformAllowInsecureHttp: platform.startsWith("http://"),
    updateUrl: `${config.downloadBaseUrl}/latest.json`,
    updatePublicKey: publicKey,
    updateAllowInsecureHttp: config.allowInsecureHttp,
  };
  if (
    existsSync(releasePath) &&
    JSON.stringify(previous) !== JSON.stringify(updated)
  )
    copyFileSync(releasePath, `${releasePath}.backup-${Date.now()}`);
  writeJson(releasePath, updated);
  chmodSync(releasePath, 0o600);
  production(config);
  console.log(
    `初始化完成。平台：${updated.platformWebUrl}\n下载/更新：${config.downloadBaseUrl}\n签名私钥仅保存在：${key}\n首次部署：pnpm release:server；之后只需 release:build 和 release:publish。`,
  );
}
function syncVersion(version) {
  const paths = [
    "package.json",
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
  ];
  const replacements = paths.map((path) => {
    const original = readFileSync(join(root, path), "utf8");
    if (path.endsWith(".json"))
      return [
        path,
        original,
        original.replace(/("version"\s*:\s*")[^"]+("\s*,)/, `$1${version}$2`),
      ];
    return [
      path,
      original,
      original.replace(/^(version\s*=\s*")[^"]+("\s*)$/m, `$1${version}$2`),
    ];
  });
  const backup = join(root, "release", ".source-backups", String(Date.now()));
  for (const [path, original, updated] of replacements) {
    if (updated !== original) {
      mkdirSync(join(backup, dirname(path)), { recursive: true });
      writeFileSync(join(backup, path), original);
      writeFileSync(join(root, path), updated);
    }
  }
  const summary = join(root, "src", "whats-new", `${version}.json`);
  if (!existsSync(summary)) writeJson(summary, { version, items: [] });
}
function defaultTarget() {
  if (process.platform === "darwin") return "macos";
  if (process.platform === "win32")
    return process.arch === "arm64" ? "windows-arm64" : "windows-x64";
  if (process.platform === "linux")
    return process.arch === "arm64" ? "linux-arm64" : "linux-x64";
  throw new Error("只支持 macOS、Windows、Linux 构建机");
}
function build(version, id) {
  const config = readPublishConfig();
  const publicConfig = production(config);
  const target = targets[id];
  if (!target || target.os !== process.platform)
    throw new Error(`目标 ${id} 必须在 ${target?.os ?? "对应"} 系统构建`);
  const env = signingEnvironment(config);
  const tag = `v${version}`;
  const names = artifactNames(id, version);
  const out = join(root, "release", tag, id);
  if (existsSync(out)) {
    if (existsSync(join(out, "build.json")))
      throw new Error(`该版本已打包：${out}。新发布请使用新版本号。`);
    renameSync(out, `${out}.failed-${Date.now()}`);
  }
  console.log(`打包 ${tag} / ${id}，将启用签名更新。`);
  syncVersion(version);
  pnpm(["typecheck"]);
  pnpm(["test:release"]);
  if (id === "macos")
    run("rustup", [
      "target",
      "add",
      "aarch64-apple-darwin",
      "x86_64-apple-darwin",
    ]);
  else run("rustup", ["target", "add", target.triple]);
  // Rename old bundles instead of scanning a directory containing stale installers.
  const targetRoot = resolve(
    root,
    process.env.CARGO_TARGET_DIR || "src-tauri/target",
  );
  const bundle = join(targetRoot, target.triple, "release", "bundle");
  if (existsSync(bundle))
    renameSync(bundle, `${bundle}.previous-${Date.now()}`);
  pnpm(
    [
      "build",
      "--",
      "--target",
      target.triple,
      "--bundles",
      target.bundles,
      "--ci",
    ],
    { env },
  );
  mkdirSync(out, { recursive: true });
  const copySigned = (from, name) => {
    copyFileSync(from, join(out, name));
    copyFileSync(`${from}.sig`, join(out, `${name}.sig`));
  };
  if (id === "macos") {
    const appDir = join(bundle, "macos");
    const app = readdirSync(appDir).filter((name) => name.endsWith(".app"));
    if (app.length !== 1) throw new Error("未找到唯一 macOS .app");
    const appPath = join(appDir, app[0]);
    if (!env.APPLE_SIGNING_IDENTITY) {
      // Internal distribution still needs a valid ad-hoc signature on Apple Silicon.
      run("codesign", [
        "--force",
        "--deep",
        "--sign",
        "-",
        "--timestamp=none",
        appPath,
      ]);
      const updater = oneFile(
        appDir,
        (path) => path.endsWith(".app.tar.gz"),
        "macOS 更新包",
      );
      run("tar", ["-czf", updater, "-C", appDir, app[0]]);
      pnpm(["tauri", "signer", "sign", updater], { env, capture: true });
    }
    run("codesign", ["--verify", "--deep", "--strict", appPath]);
    run("ditto", [
      "-c",
      "-k",
      "--sequesterRsrc",
      "--keepParent",
      appPath,
      join(out, names.zip),
    ]);
    // hdiutil does not depend on Finder, unlike Tauri's DMG decoration script.
    run("hdiutil", [
      "create",
      "-volname",
      "Zhishu CCS",
      "-srcfolder",
      appPath,
      "-format",
      "UDZO",
      join(out, names.dmg),
    ]);
    copySigned(
      oneFile(appDir, (path) => path.endsWith(".app.tar.gz"), "macOS 更新包"),
      names["tar.gz"],
    );
  } else if (target.platform === "windows") {
    copySigned(
      oneFile(bundle, (path) => path.endsWith(".msi"), "Windows MSI"),
      names.msi,
    );
  } else {
    for (const [ext, name] of Object.entries(names))
      copySigned(
        oneFile(bundle, (path) => path.endsWith(`.${ext}`), `Linux ${ext}`),
        name,
      );
  }
  // Verify in place before marking the build complete; no duplicate installers.
  collectUpdaterPlatforms(out, [id], version, publicConfig, config);
  saveReceipt(out, id, version, publicConfig);
  console.log(`打包完成：${out}\n发布：pnpm release:publish ${version}`);
}
function installServer(config) {
  ssh(
    config,
    `command -v python3 >/dev/null && mkdir -p ${quote(config.remoteRoot + "/tools")} ${quote(config.remoteRoot + "/versions")} ${quote(config.remoteRoot + "/.staging")}`,
  );
  scp(
    config,
    join(root, "deploy/client-download/server.py"),
    `${config.remoteRoot}/tools/server.py.new`,
  );
  ssh(
    config,
    `mv ${quote(config.remoteRoot + "/tools/server.py.new")} ${quote(config.remoteRoot + "/tools/server.py")} && chmod 755 ${quote(config.remoteRoot)} ${quote(config.remoteRoot + "/versions")} && chmod 644 ${quote(config.remoteRoot + "/tools/server.py")}`,
  );
  console.log(
    `发布目录已准备：${config.remoteRoot}。下载通过平台 ${config.downloadBaseUrl} 提供，无需新增端口。`,
  );
}
export async function checkDownloadService(config, fetcher = fetch) {
  const response = await fetcher(
    `${config.downloadBaseUrl}/__release_probe__`,
    {
      cache: "no-store",
      signal: AbortSignal.timeout(15000),
    },
  );
  if (response.status !== 404) {
    throw new Error(
      "网关下载目录尚未就绪：未上传、未切换线上版本。先阅读文档的一次性平台准备，再运行 pnpm release:check。",
    );
  }
  console.log("下载目录探测通过；此检查未修改服务器。");
}
async function fetchChecked(url, { head = false, expected } = {}) {
  const response = await fetch(url, {
    method: head ? "HEAD" : "GET",
    cache: "no-store",
    signal: AbortSignal.timeout(20000),
  });
  if (!response.ok)
    throw new Error(`公网验证失败：${url} (HTTP ${response.status})`);
  if (head) {
    if (Number(response.headers.get("content-length")) !== expected.size)
      throw new Error(`下载大小不符：${url}`);
  } else if (
    hashFileFromBuffer(Buffer.from(await response.arrayBuffer())) !==
    expected.sha256
  )
    throw new Error(`线上清单与本地不一致：${url}`);
}
function hashFileFromBuffer(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}
async function publish(version, config, dryRun) {
  const publicConfig = production(config);
  const versionRoot = join(root, "release", `v${version}`);
  if (!existsSync(versionRoot))
    throw new Error("没有打包产物，先执行 pnpm release:build");
  const dirs = readdirSync(versionRoot)
    .filter(
      (id) => targets[id] && existsSync(join(versionRoot, id, "build.json")),
    )
    .map((id) => join(versionRoot, id));
  if (!dirs.length) throw new Error("没有完整构建记录");
  const receipts = dirs.map((dir) =>
    validateReceipt(dir, version, publicConfig),
  );
  const inputs = receipts.map((receipt) => ({
    target: receipt.target,
    files: receipt.files,
  }));
  const inputHash = configFingerprint({
    config: publicConfig,
    download: config.downloadBaseUrl,
    inputs,
  });
  const out = join(versionRoot, "publish");
  const statePath = join(versionRoot, "publish-inputs.json");
  if (existsSync(out)) {
    if (!existsSync(statePath) || json(statePath).inputHash !== inputHash)
      throw new Error(
        "安装包已变更，旧 publish 目录先移走；已上线版本请使用新版本号",
      );
  } else {
    assembleRelease(dirs, out, version, publicConfig, config);
    writeJson(statePath, { inputHash });
  }
  const metadata = json(join(out, "release.json"));
  for (const file of metadata.files)
    if (hashFile(join(out, file.name)) !== file.sha256)
      throw new Error(`发布文件损坏：${file.name}`);
  console.log(
    `发布 ${metadata.tag}，平台：${metadata.targets.join(", ")}\n安装包地址：${config.downloadBaseUrl}/${metadata.tag}/\n平台下载清单：${config.proxyManifest}`,
  );
  if (dryRun) {
    console.log("预演完成：未连接服务器、未上传、未切换版本。");
    return;
  }
  await checkDownloadService(config);
  const stage = `${config.remoteRoot}/.staging/${metadata.tag}-${randomUUID()}`;
  ssh(
    config,
    `test -f ${quote(config.remoteRoot + "/tools/server.py")} && mkdir -p ${quote(stage)}`,
  );
  scp(config, out, `${stage}/payload`, true);
  const command = (mode, dir) =>
    [
      "python3",
      `${config.remoteRoot}/tools/server.py`,
      mode,
      config.remoteRoot,
      dir,
      config.proxyManifest,
    ]
      .map(quote)
      .join(" ");
  ssh(config, command("prepare", `${stage}/payload`));
  // Check real public delivery before making the new version discoverable.
  for (const file of metadata.files.filter(
    (file) => !file.name.endsWith(".json") && !file.name.endsWith(".sig"),
  ))
    await fetchChecked(
      `${config.downloadBaseUrl}/${metadata.tag}/${encodeURIComponent(file.name)}`,
      { head: true, expected: file },
    );
  ssh(
    config,
    command("publish", `${config.remoteRoot}/versions/${metadata.tag}`),
  );
  for (const name of ["latest.json", "client-release.json"])
    await fetchChecked(`${config.downloadBaseUrl}/${name}`, {
      expected: metadata.files.find((file) => file.name === name),
    });
  await fetchChecked(`${publicConfig.platformWebUrl}/client-release.json`, {
    expected: metadata.files.find(
      (file) => file.name === "client-release.json",
    ),
  });
  console.log(
    `发布并验证成功：${metadata.tag}\n网页下载：${publicConfig.platformWebUrl}/download\n自动更新：${config.downloadBaseUrl}/latest.json`,
  );
}
function importBuild(from, version) {
  const config = production(readPublishConfig());
  const receipt = validateReceipt(resolve(from), version, config);
  const out = join(root, "release", `v${version}`, receipt.target);
  if (existsSync(out)) throw new Error(`目标产物已存在：${out}`);
  mkdirSync(out, { recursive: true });
  for (const name of [...receipt.files.map((file) => file.name), "build.json"])
    copyFileSync(join(resolve(from), name), join(out, name));
  console.log(`已汇总 ${receipt.target}：${out}`);
}
export async function main(args = process.argv.slice(2)) {
  const command = args.shift();
  if (["help", "--help", "-h"].includes(command)) {
    console.log(`智枢客户端发布（在客户端仓库执行）
本地初始化：pnpm release:init
1 打包：pnpm release:build <新版本号>
2 发布：pnpm release:publish
本地预演：pnpm release:publish --dry-run
只读检查：pnpm release:check
首次上传发布工具：pnpm release:server
汇总其他系统：pnpm release:import <版本号> <产物目录>
指定目标：release:build <版本> --target macos|windows-x64|windows-arm64|linux-x64|linux-arm64
打包和预演不连接服务器；发布只上传客户端静态文件和清单，不部署或重启网关。
首次平台接入与执行顺序见 docs/zhishu-release.md。`);
    return;
  }

  const dryRun = args.includes("--dry-run");
  args = args.filter((arg) => arg !== "--" && arg !== "--dry-run");
  const targetAt = args.indexOf("--target");
  let target;
  if (targetAt >= 0) {
    target = args[targetAt + 1];
    if (!target || !targets[target]) throw new Error("未知 --target");
    args.splice(targetAt, 2);
  }
  if (command === "init" && args.length === 0 && !dryRun && !target)
    return initialize();
  if (command === "server" && args.length === 0 && !dryRun && !target)
    return installServer(readPublishConfig());
  if (command === "check" && args.length === 0 && !dryRun && !target)
    return checkDownloadService(readPublishConfig());
  const version = versionNumber(
    args[0] ?? json(join(root, "package.json")).version,
  );
  if (command === "build" && args.length <= 1 && !dryRun)
    return build(version, target ?? defaultTarget());
  if (command === "publish" && args.length <= 1 && !target)
    return publish(version, readPublishConfig(), dryRun);
  if (command === "import" && args.length === 2 && !target && !dryRun)
    return importBuild(args[1], version);
  throw new Error(
    "用法：pnpm release:init | release:server | release:check | release:build [版本] [--target macos|windows-x64|windows-arm64|linux-x64|linux-arm64] | release:publish [版本] [--dry-run] | release:import <版本> <构建目录>",
  );
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error) => {
    console.error(`发布失败：${error.message}`);
    process.exitCode = 1;
  });
