#!/usr/bin/env node
// Prepare a release commit and push its branch/tag; building and publishing stay in Actions.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { parse as parseToml } from "smol-toml";
import { checkVersion } from "./cos-release.mjs";
import { versionNumber } from "./release-artifacts.mjs";

const remote = "zhishu-git";
const versionFiles = [
  "package.json",
  "src-tauri/tauri.conf.json",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
];

export const help = `用法：pnpm release:tag [patch|minor|major|vX.Y.Z] [--dry-run]
      pnpm release:tag --tag vX.Y.Z [--dry-run]
      pnpm release:tag --version X.Y.Z [--dry-run]

默认递增 patch；--bump patch|minor|major 也可指定递增方式。
--dry-run 只预览，不修改文件、不联网、不提交或推送。
正式执行要求先提交现有改动；随后自动同步版本、检查、提交并推送。
推送目标固定为 zhishu-git（zws-code/zhishu-ccs），会触发正式发布。`;

export function parseArgs(args) {
  const options = { bump: "patch", dryRun: false, help: false };
  let selected = false;
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--") continue;
    if (arg === "--dry-run") options.dryRun = true;
    else if (arg === "--help" || arg === "-h") options.help = true;
    else {
      if (selected) throw new Error("只能指定一种版本号或递增方式");
      selected = true;
      const value = ["--tag", "--version", "--bump"].includes(arg)
        ? args[++i]
        : arg;
      if (!value) throw new Error(`${arg} 缺少参数`);
      if (arg === "--bump" || ["patch", "minor", "major"].includes(value)) {
        if (!["patch", "minor", "major"].includes(value))
          throw new Error("递增方式必须是 patch、minor 或 major");
        if (arg === "--tag" || arg === "--version")
          throw new Error(`${arg} 必须指定正式版本号`);
        options.bump = value;
      } else {
        if (arg === "--tag" && !value.startsWith("v"))
          throw new Error("--tag 必须使用 vX.Y.Z");
        options.version = versionNumber(value);
      }
    }
  }
  return options;
}

function compareVersions(a, b) {
  const left = a.split(".").map(BigInt);
  const right = b.split(".").map(BigInt);
  for (let i = 0; i < 3; i++) {
    if (left[i] !== right[i]) return left[i] > right[i] ? 1 : -1;
  }
  return 0;
}

export function nextVersion(current, options) {
  versionNumber(current);
  let version = options.version;
  if (!version) {
    const parts = current.split(".").map(BigInt);
    const index = ["major", "minor", "patch"].indexOf(options.bump || "patch");
    if (index < 0) throw new Error("未知递增方式");
    parts[index]++;
    for (let i = index + 1; i < 3; i++) parts[i] = 0n;
    version = parts.join(".");
  }
  versionNumber(version);
  if (compareVersions(version, current) <= 0)
    throw new Error(`新版本 ${version} 必须高于当前版本 ${current}`);
  return version;
}

export function runCommand(command, args, options) {
  // pnpm on Windows is a .cmd shim; npm_execpath lets us avoid shell interpolation.
  if (command === "pnpm" && process.env.npm_execpath) {
    args = [process.env.npm_execpath, ...args];
    command = process.execPath;
  }
  const result = spawnSync(command, args, {
    ...options,
    encoding: "utf8",
    shell: false,
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(
      `${command} ${args.join(" ")} 执行失败（${result.status}）\n${result.stderr || ""}`.trim(),
    );
  return result.stdout || "";
}

export function tagRelease({
  root = fileURLToPath(new URL("../../", import.meta.url)),
  options = {},
  run = runCommand,
  log = console.log,
} = {}) {
  const execute = (command, args, extra = {}) =>
    run(command, args, { cwd: root, ...extra });
  const git = (...args) => execute("git", args).trim();
  const read = (file) => readFileSync(join(root, file), "utf8");
  const write = (file, text) => writeFileSync(join(root, file), text);
  const originalHead = git("rev-parse", "HEAD");
  const pkg = JSON.parse(read("package.json"));
  const current = versionNumber(pkg.version);
  checkVersion(root, `v${current}`);
  const cargo = parseToml(read("src-tauri/Cargo.toml"));
  const lock = parseToml(read("src-tauri/Cargo.lock"));
  const own = lock.package?.filter(
    (entry) => entry.name === cargo.package.name,
  );
  if (own?.length !== 1 || own[0].version !== current)
    throw new Error("Cargo.lock 的应用版本不一致，请先用 Cargo 同步并提交");
  const version = nextVersion(current, options);
  const tag = `v${version}`;
  const branch = `codex/release-${version}`;
  const files = [...versionFiles, `src/whats-new/${version}.json`];
  const refspecs = (sha) => [
    `${sha}:refs/heads/${branch}`,
    `refs/tags/${tag}:refs/tags/${tag}`,
  ];
  const verifyRemote = () => {
    const urls = git("remote", "get-url", "--push", "--all", remote).split(
      /\r?\n/,
    );
    if (
      urls.length !== 1 ||
      ![
        "git@github.com:zws-code/zhishu-ccs.git",
        "https://github.com/zws-code/zhishu-ccs.git",
        "ssh://git@github.com/zws-code/zhishu-ccs.git",
      ].includes(urls[0])
    )
      throw new Error(
        "zhishu-git 必须只有一个 push URL，指向 zws-code/zhishu-ccs；请核对 remote",
      );
    return urls[0];
  };
  if (realpathSync(git("rev-parse", "--show-toplevel")) !== realpathSync(root))
    throw new Error("请在智枢客户端仓库运行");
  const pushUrl = verifyRemote();
  log(`${current} → ${version}；tag: ${tag}；发布分支: ${branch}`);
  log(`版本文件：${files.join("、")}`);
  if (options.dryRun) {
    log(
      "仅预览：正式执行会检查工作区和远程 tag，运行 Cargo、类型/发布/公告回归，再提交并原子推送分支与 tag。",
    );
    return { version, tag, branch, dryRun: true };
  }

  const ensureClean = () => {
    if (git("status", "--porcelain", "--untracked-files=all"))
      throw new Error(
        "工作区有未提交改动（含未跟踪文件），请先审查并提交；脚本不会自动加入业务文件",
      );
  };
  ensureClean();
  git("symbolic-ref", "--quiet", "HEAD");
  for (const state of [
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "rebase-merge",
    "rebase-apply",
  ]) {
    if (existsSync(resolve(root, git("rev-parse", "--git-path", state))))
      throw new Error("请先完成当前 merge/rebase/cherry-pick/revert");
  }
  git("ls-files", "--error-unmatch", "--", ...versionFiles);
  if (git("tag", "--list", tag))
    throw new Error(`本地 tag ${tag} 已存在，不能覆盖`);
  if (
    git(
      "ls-remote",
      "--refs",
      pushUrl,
      `refs/tags/${tag}`,
      `refs/heads/${branch}`,
    )
  )
    throw new Error(
      `远程 tag ${tag} 或发布分支已存在，不能重复发布；失败发布请复用原产物`,
    );
  const originalBranch = git("symbolic-ref", "HEAD");
  // Network preflight can take time: recheck before touching any files.
  ensureClean();
  if (git("rev-parse", "HEAD") !== originalHead)
    throw new Error("预检期间 HEAD 发生变化，请重新运行");
  let committed;
  let tagged = false;
  try {
    for (const file of versionFiles.slice(0, 2)) {
      // Replace only the top-level version value; preserve the rest of the formatting.
      const text = read(file);
      const changed = text.replace(
        /^(\s*"version"\s*:\s*")[^"]+("\s*[,}]?)/m,
        (_, before, after) => `${before}${version}${after}`,
      );
      if (changed === text) throw new Error(`${file} 未找到可更新的 version`);
      write(file, changed);
    }
    const manifest = read("src-tauri/Cargo.toml");
    const changed = manifest.replace(
      /(\[package\][\s\S]*?^version\s*=\s*")[^"]+(".*$)/m,
      (_, before, after) => `${before}${version}${after}`,
    );
    if (changed === manifest)
      throw new Error("Cargo.toml 未找到 [package] version");
    write("src-tauri/Cargo.toml", changed);
    log("使用 Cargo 离线同步锁文件…");
    execute(
      "cargo",
      [
        "update",
        "--workspace",
        "--offline",
        "--manifest-path",
        "src-tauri/Cargo.toml",
      ],
      { stdio: "inherit" },
    );
    own[0].version = version;
    if (!isDeepStrictEqual(parseToml(read("src-tauri/Cargo.lock")), lock))
      throw new Error(
        "Cargo.lock 出现应用版本以外的变化；已停止，请审查依赖变化后再发布",
      );
    if (!existsSync(join(root, files[4])))
      write(files[4], JSON.stringify({ version, items: [] }, null, 2) + "\n");
    const summary = JSON.parse(read(files[4]));
    if (summary.version !== version || !Array.isArray(summary.items))
      throw new Error("更新公告的 version/items 不合法");
    checkVersion(root, tag);
    const prepared = new Map(files.map((file) => [file, read(file)]));
    for (const args of [
      ["typecheck"],
      ["test:release"],
      ["test:release-cos"],
      ["exec", "vitest", "run", "tests/config/whatsNewEntries.test.ts"],
    ]) {
      log(`执行 pnpm ${args.join(" ")}…`);
      execute("pnpm", args, { stdio: "inherit" });
    }
    git("diff", "--check");
    if (
      git("rev-parse", "HEAD") !== originalHead ||
      git("symbolic-ref", "HEAD") !== originalBranch
    )
      throw new Error("检查期间 HEAD 或分支发生变化，请重新核对发布内容");
    const changes = [
      ...git("diff", "--name-only", "-z", "HEAD").split("\0"),
      ...git("ls-files", "--others", "--exclude-standard", "-z").split("\0"),
    ].filter(Boolean);
    if (
      changes.some((file) => !files.includes(file)) ||
      files.some((file) => read(file) !== prepared.get(file))
    )
      throw new Error("检查期间文件发生非预期变化；请审查后重新发布");
    git("add", "--", ...files);
    git("commit", "-m", `chore(release): prepare ${version}`);
    committed = git("rev-parse", "HEAD");
    ensureClean();
    if (
      git("rev-parse", "HEAD^") !== originalHead ||
      files.some(
        (file) => git("show", `HEAD:${file}`) !== prepared.get(file).trim(),
      )
    )
      throw new Error("提交钩子改变了发布内容，已停止打 tag");
    git("tag", "-a", tag, "-m", `Zhishu CCS ${tag}`, committed);
    tagged = true;
    if (verifyRemote() !== pushUrl)
      throw new Error("发布期间 push URL 发生变化");
    // Ignore a user's push.followTags=true: only this release tag may be sent.
    git("push", "--atomic", "--no-follow-tags", remote, ...refspecs(committed));
    log(`已推送 ${tag}，GitHub Actions 将构建并发布 COS/GitHub Release。`);
    log("https://github.com/zws-code/zhishu-ccs/actions/workflows/release.yml");
    return { version, tag, branch, commit: committed };
  } catch (error) {
    if (committed) {
      log(
        `保留发布提交 ${committed}${tagged ? ` 和 tag ${tag}` : ""}。请先核对远程结果，不要移动 tag 或重新生成同版本包。`,
      );
      if (tagged)
        log(
          `仅需重试推送时：git push --atomic --no-follow-tags ${remote} ${refspecs(committed).join(" ")}`,
        );
    } else {
      log(
        "已停止发布，保留本地版本文件供审查；尚未打 tag 或推送。修复后可按发布文档完成该版本，或仅还原本次版本改动再重试。",
      );
    }
    throw error;
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) console.log(help);
    else tagRelease({ options });
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
