import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import {
  parseArgs,
  nextVersion,
  runCommand,
  tagRelease,
} from "./tag-release.mjs";

const github = "git@github.com:zws-code/zhishu-ccs.git";
const read = (root, file) => readFileSync(join(root, file), "utf8");
const write = (root, file, content) => {
  mkdirSync(dirname(join(root, file)), { recursive: true });
  writeFileSync(join(root, file), content);
};

function fixture(t) {
  const dir = mkdtempSync(join(tmpdir(), "zhishu-tag-release-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const root = join(dir, "repo with spaces");
  const bare = join(dir, "github.git");
  mkdirSync(root);
  // Use only repo-local identity/config; never change HOME or CODEX_HOME.
  const env = {
    ...process.env,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: process.platform === "win32" ? "NUL" : "/dev/null",
  };
  const git = (...args) => runCommand("git", args, { cwd: root, env }).trim();
  git("init", "--initial-branch=codex/test");
  git("config", "user.name", "Release Test");
  git("config", "user.email", "release-test@example.invalid");
  git("config", "commit.gpgsign", "false");
  git("config", "tag.gpgsign", "false");
  git("config", "core.hooksPath", join(dir, "no-hooks"));
  git("init", "--bare", bare);
  git("remote", "add", "zhishu-git", github);
  write(
    root,
    "package.json",
    '{\n  "name": "zhishu-ccs",\n  "version": "0.0.3"\n}\n',
  );
  write(
    root,
    "src-tauri/tauri.conf.json",
    '{\n  "version": "0.0.3",\n  "app": { "inline": true },\n  "identifier": "com.zhishu.ccswitch.desktop"\n}\n',
  );
  write(
    root,
    "src-tauri/Cargo.toml",
    '[package]\nname = "zhishu-ccs"\nversion = "0.0.3" # keep comment\nedition = "2021"\n\n[dependencies]\n',
  );
  write(
    root,
    "src-tauri/Cargo.lock",
    'version = 4\n\n[[package]]\nname = "zhishu-ccs"\nversion = "0.0.3"\n',
  );
  write(root, "src/whats-new/0.0.3.json", '{"version":"0.0.3","items":[]}\n');
  git("add", ".");
  git("commit", "-m", "fixture");
  const initial = git("rev-parse", "HEAD");
  const calls = [];
  const logs = [];
  const state = { fail: null, dependencyDrift: false, afterChecks: null };
  const run = (command, args, options) => {
    calls.push({ command, args: [...args] });
    if (state.fail?.(command, args)) throw new Error("injected failure");
    if (command === "cargo") {
      // Model Cargo's isolated workspace version rewrite; actual Cargo is tested separately.
      const version = read(root, "src-tauri/Cargo.toml").match(
        /^version = "([^"]+)"/m,
      )[1];
      write(
        root,
        "src-tauri/Cargo.lock",
        `version = 4\n\n[[package]]\nname = "zhishu-ccs"\nversion = "${version}"\n${state.dependencyDrift ? '\n[[package]]\nname = "unexpected"\nversion = "1.0.0"\n' : ""}`,
      );
      return "";
    }
    if (command === "pnpm") {
      if (args[0] === "exec") state.afterChecks?.();
      return "";
    }
    // Git really commits, tags and pushes, but every network operation is routed
    // explicitly to this fixture's bare repo. No real account/service is accessed.
    const actual = [...args];
    if (actual[0] === "ls-remote") actual[2] = bare;
    if (actual[0] === "push") actual[3] = bare;
    return runCommand(command, actual, { ...options, env });
  };
  const release = (args = []) =>
    tagRelease({
      root,
      options: parseArgs(args),
      run,
      log: (line) => logs.push(line),
    });
  const remoteRefs = () => git("ls-remote", "--refs", bare);
  return { root, bare, git, initial, calls, logs, state, release, remoteRefs };
}

test("defaults to patch, supports bumps and explicit tag/version spellings", () => {
  for (const [args, expected] of [
    [[], "0.0.4"],
    [["minor"], "0.1.0"],
    [["--bump", "major"], "1.0.0"],
    [["v0.2.3"], "0.2.3"],
    [["--tag", "v0.2.3"], "0.2.3"],
    [["--version", "0.2.3"], "0.2.3"],
    [["--", "--tag", "v0.2.3"], "0.2.3"],
  ])
    assert.equal(nextVersion("0.0.3", parseArgs(args)), expected);
  for (const args of [
    ["--tag"],
    ["--tag", "0.0.4"],
    ["--tag", "v0.0.4-beta.1"],
    ["--version", "0.00.4"],
    ["--version", "patch"],
    ["--bump", "other"],
    ["--unknown"],
    ["minor", "v1.0.0"],
    ["--tag", "v0.0.4;touch /tmp/no"],
  ])
    assert.throws(() => parseArgs(args));
  for (const version of ["0.0.3", "0.0.2", "0.0.0"])
    assert.throws(() => nextVersion("0.0.3", { version }), /必须高于/);
});

test("dry-run is offline and leaves dirty files, refs and versions untouched", (t) => {
  const f = fixture(t);
  write(f.root, "business.txt", "uncommitted work");
  const before = f.git("status", "--porcelain");
  const result = f.release(["--tag", "v0.1.0", "--dry-run"]);
  assert.equal(result.version, "0.1.0");
  assert.equal(f.git("status", "--porcelain"), before);
  assert.equal(f.git("rev-parse", "HEAD"), f.initial);
  assert.equal(
    f.calls.some(
      ({ command, args }) =>
        command !== "git" ||
        ["ls-remote", "push", "add", "commit", "tag"].includes(args[0]),
    ),
    false,
  );
});

test("explicit tag updates only release files, commits and atomically pushes matching refs", (t) => {
  const f = fixture(t);
  f.git("config", "push.followTags", "true");
  f.git("tag", "-a", "v0.0.3", "-m", "old release");
  const result = f.release(["--tag", "v0.1.0"]);
  assert.equal(result.tag, "v0.1.0");
  assert.equal(JSON.parse(read(f.root, "package.json")).version, "0.1.0");
  assert.match(
    read(f.root, "src-tauri/Cargo.toml"),
    /version = "0.1.0" # keep comment/,
  );
  assert.match(read(f.root, "src-tauri/Cargo.lock"), /version = "0.1.0"/);
  assert.match(
    read(f.root, "src-tauri/tauri.conf.json"),
    /"app": \{ "inline": true \}/,
  );
  assert.deepEqual(JSON.parse(read(f.root, "src/whats-new/0.1.0.json")), {
    version: "0.1.0",
    items: [],
  });
  assert.equal(f.git("status", "--porcelain"), "");
  assert.equal(f.git("cat-file", "-t", result.tag), "tag");
  assert.equal(f.git("rev-parse", `${result.tag}^{commit}`), result.commit);
  assert.equal(f.git("rev-parse", "HEAD^"), f.initial);
  assert.match(
    f.remoteRefs(),
    new RegExp(`${result.commit}\\s+refs/heads/codex/release-0.1.0`),
  );
  assert.match(f.remoteRefs(), /refs\/tags\/v0.1.0/);
  assert.doesNotMatch(f.remoteRefs(), /refs\/tags\/v0.0.3/);
  const push = f.calls.find(({ args }) => args[0] === "push");
  assert.deepEqual(push.args, [
    "push",
    "--atomic",
    "--no-follow-tags",
    "zhishu-git",
    `${result.commit}:refs/heads/codex/release-0.1.0`,
    "refs/tags/v0.1.0:refs/tags/v0.1.0",
  ]);
  assert.equal(f.calls.filter(({ command }) => command === "pnpm").length, 4);
});

test("keeps a prepared and committed announcement", (t) => {
  const f = fixture(t);
  const summary =
    '{"version":"0.0.4","items":[{"type":"fix","zh":"已有公告"}]}\n';
  write(f.root, "src/whats-new/0.0.4.json", summary);
  f.git("add", ".");
  f.git("commit", "-m", "prepare announcement");
  f.release();
  assert.equal(read(f.root, "src/whats-new/0.0.4.json"), summary);
});

for (const mode of [
  "dirty",
  "wrong-remote",
  "multiple-push-urls",
  "local-tag",
  "remote-tag",
  "remote-branch",
  "detached",
  "merge-state",
  "lock-mismatch",
  "source-mismatch",
]) {
  test(`rejects ${mode} before modifying version files or publishing`, (t) => {
    const f = fixture(t);
    if (mode === "dirty") write(f.root, "business.txt", "keep me");
    if (mode === "wrong-remote")
      f.git(
        "remote",
        "set-url",
        "zhishu-git",
        "git@github.com:farion1231/cc-switch.git",
      );
    if (mode === "multiple-push-urls") {
      f.git("remote", "set-url", "--add", "--push", "zhishu-git", github);
      f.git(
        "remote",
        "set-url",
        "--add",
        "--push",
        "zhishu-git",
        "git@github.com:other/repo.git",
      );
    }
    if (mode === "local-tag") f.git("tag", "v0.0.4");
    if (mode === "remote-tag") f.git("push", f.bare, "HEAD:refs/tags/v0.0.4");
    if (mode === "remote-branch")
      f.git("push", f.bare, "HEAD:refs/heads/codex/release-0.0.4");
    if (mode === "detached") f.git("checkout", "--detach");
    if (mode === "merge-state")
      writeFileSync(
        resolve(f.root, f.git("rev-parse", "--git-path", "MERGE_HEAD")),
        f.initial + "\n",
      );
    if (mode === "lock-mismatch" || mode === "source-mismatch") {
      const file =
        mode === "lock-mismatch"
          ? "src-tauri/Cargo.lock"
          : "src-tauri/tauri.conf.json";
      write(f.root, file, read(f.root, file).replace("0.0.3", "0.0.2"));
      f.git("add", ".");
      f.git("commit", "-m", "inconsistent fixture");
    }
    const pkg = read(f.root, "package.json");
    const head = f.git("rev-parse", "HEAD");
    assert.throws(() => f.release());
    assert.equal(read(f.root, "package.json"), pkg);
    assert.equal(f.git("rev-parse", "HEAD"), head);
    assert.equal(
      f.calls.some(({ command }) => command === "cargo"),
      false,
    );
    assert.equal(
      f.calls.some(({ args }) => args[0] === "push"),
      false,
    );
  });
}

for (const mode of [
  "cargo-failure",
  "dependency-drift",
  "failed-check",
  "concurrent-edit",
  "concurrent-version-edit",
  "invalid-summary",
]) {
  test(`stops ${mode} without committing, tagging or pushing`, (t) => {
    const f = fixture(t);
    if (mode === "cargo-failure")
      f.state.fail = (command) => command === "cargo";
    if (mode === "dependency-drift") f.state.dependencyDrift = true;
    if (mode === "failed-check") f.state.fail = (command) => command === "pnpm";
    if (mode === "concurrent-edit")
      f.state.afterChecks = () => write(f.root, "other.txt", "keep this edit");
    if (mode === "concurrent-version-edit")
      f.state.afterChecks = () =>
        write(f.root, "package.json", '{"version":"9.9.9"}\n');
    if (mode === "invalid-summary") {
      write(
        f.root,
        "src/whats-new/0.0.4.json",
        '{"version":"0.0.5","items":[]}\n',
      );
      f.git("add", ".");
      f.git("commit", "-m", "invalid summary fixture");
    }
    const head = f.git("rev-parse", "HEAD");
    assert.throws(() => f.release());
    assert.equal(f.git("rev-parse", "HEAD"), head);
    assert.equal(f.git("tag", "--list", "v0.0.4"), "");
    assert.equal(f.remoteRefs(), "");
    if (mode === "concurrent-edit")
      assert.equal(read(f.root, "other.txt"), "keep this edit");
  });
}

test("atomic push rejection preserves the local commit/tag and prints exact retry command", (t) => {
  const f = fixture(t);
  // A remote without atomic push support rejects both refs together.
  f.git("--git-dir", f.bare, "config", "receive.advertiseAtomic", "false");
  assert.throws(() => f.release(), /执行失败/);
  assert.notEqual(f.git("rev-parse", "HEAD"), f.initial);
  assert.equal(
    f.git("rev-parse", "v0.0.4^{commit}"),
    f.git("rev-parse", "HEAD"),
  );
  assert.equal(f.remoteRefs(), "");
  assert.match(
    f.logs.join("\n"),
    /仅需重试推送时：git push --atomic --no-follow-tags zhishu-git/,
  );
});

test("real Cargo offline update synchronizes a minimal workspace lock without compilation", (t) => {
  const f = fixture(t);
  write(f.root, "src-tauri/src/main.rs", "fn main() {}\n");
  const result = spawnSync(
    "cargo",
    [
      "update",
      "--workspace",
      "--offline",
      "--manifest-path",
      "src-tauri/Cargo.toml",
    ],
    { cwd: f.root, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  write(
    f.root,
    "src-tauri/Cargo.toml",
    read(f.root, "src-tauri/Cargo.toml").replace('"0.0.3"', '"0.0.4"'),
  );
  const updated = spawnSync(
    "cargo",
    [
      "update",
      "--workspace",
      "--offline",
      "--manifest-path",
      "src-tauri/Cargo.toml",
    ],
    { cwd: f.root, encoding: "utf8" },
  );
  assert.equal(updated.status, 0, updated.stderr);
  assert.match(
    read(f.root, "src-tauri/Cargo.lock"),
    /name = "zhishu-ccs"\nversion = "0.0.4"/,
  );
  assert.equal(existsSync(join(f.root, "src-tauri/target")), false);
});
