import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  resolveIdentity,
  selectIdentity,
  signExecutable,
} from "./macos-dev-sign.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const hash = "A".repeat(40);
const otherHash = "B".repeat(40);
const name = "Apple Development: Test (TEAM123456)";
const identity = `  1) ${hash} "${name}"\n  1 valid identities found\n`;
const details =
  "Identifier=com.zhishu.ccswitch.desktop\nTeamIdentifier=TEAM123456\n";

test("prefers a valid Apple development identity over self-signed and distribution identities", () => {
  assert.deepEqual(
    selectIdentity(
      `${identity}  2) ${otherHash} "Zhishu CC Switch Dev"\n  3) ${"C".repeat(40)} "Developer ID Application: Test (TEAM123456)"\n`,
    ),
    { hash, name },
  );
});

test("rejects missing, invalid, self-signed and ambiguous identities", () => {
  for (const output of [
    "0 valid identities found",
    `  1) ${hash} "Zhishu CC Switch Dev"\n`,
    `  1) ${hash} "${name}" (CSSMERR_TP_CERT_EXPIRED)\n`,
  ]) {
    // Invalid identity lines have a trailing error rather than a bare quoted name.
    assert.throws(() => selectIdentity(output), /No matching valid Apple/);
  }
  assert.throws(
    () =>
      selectIdentity(
        `${identity}  2) ${otherHash} "Apple Development: Other (OTHER12345)"\n`,
      ),
    /Multiple signing identities/,
  );
});

test("accepts exact names and fingerprints and deduplicates the same identity", () => {
  assert.deepEqual(selectIdentity(identity, name), { hash, name });
  assert.deepEqual(selectIdentity(identity, hash.toLowerCase()), {
    hash,
    name,
  });
  assert.deepEqual(selectIdentity(identity + identity), { hash, name });
  assert.throws(() => selectIdentity(identity, "Test"), /No matching/);
});

function fakeSigner(failAt = -1, signedDetails = details) {
  const calls = [];
  return {
    calls,
    run(binary, args, options) {
      calls.push({ binary, args, options });
      if (calls.length === failAt) return { status: 1, stderr: "test failure" };
      return {
        status: 0,
        stdout: calls.length === 1 ? identity : "",
        stderr: calls.length === 4 ? signedDetails : "",
      };
    },
  };
}

test("signs by fingerprint, verifies Apple trust and checks the team before returning", () => {
  const { calls, run } = fakeSigner();
  assert.equal(
    signExecutable("/tmp/path with spaces/app", {}, run).team,
    "TEAM123456",
  );
  assert.deepEqual(calls[0].args, ["find-identity", "-v", "-p", "codesigning"]);
  assert.deepEqual(calls[1].args, [
    "--force",
    "--sign",
    hash,
    "--identifier",
    "com.zhishu.ccswitch.desktop",
    "--timestamp=none",
    "/tmp/path with spaces/app",
  ]);
  assert.deepEqual(calls[2].args, [
    "--verify",
    "--strict",
    "-R=anchor apple generic",
    "/tmp/path with spaces/app",
  ]);
});

test("signing and verification failures never fall back to an ad-hoc launch", () => {
  for (const stage of [1, 2, 3, 4]) {
    const { calls, run } = fakeSigner(stage);
    assert.throws(() => signExecutable("/tmp/app", {}, run), /test failure/);
    assert.equal(calls.length, stage);
  }
  for (const invalid of [
    "TeamIdentifier=not set\n",
    "Identifier=other\nTeamIdentifier=TEAM123456\n",
  ]) {
    const { run } = fakeSigner(-1, invalid);
    assert.throws(
      () => signExecutable("/tmp/app", {}, run),
      /refusing to launch/,
    );
  }
});

function fixture(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "zhishu-dev-sign-"));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  return dir;
}

test(
  "Cargo runner stops on signer failure and preserves application arguments and exit status",
  { skip: process.platform === "win32" },
  (t) => {
    const dir = fixture(t);
    const node = path.join(dir, "node");
    const app = path.join(dir, "test app");
    fs.writeFileSync(node, "#!/bin/sh\nexit 42\n", { mode: 0o700 });
    fs.writeFileSync(app, "#!/bin/sh\nprintf '<%s>\\n' \"$@\"\nexit 17\n", {
      mode: 0o700,
    });
    const args = [
      path.join(root, "scripts/macos-dev-sign-runner.sh"),
      app,
      "hello world",
      "--flag",
    ];
    const options = {
      env: { ...process.env, PATH: `${dir}:/usr/bin:/bin` },
      encoding: "utf8",
    };
    let result = spawnSync("/bin/sh", args, options);
    assert.equal(result.status, 42);
    assert.equal(result.stdout, "");
    fs.writeFileSync(node, "#!/bin/sh\nexit 0\n");
    result = spawnSync("/bin/sh", args, options);
    assert.equal(result.status, 17);
    assert.equal(result.stdout, "<hello world>\n<--flag>\n");
  },
);

// Opt in explicitly: this test creates and deletes one dummy Keychain item.
test(
  "real macOS Keychain access survives rebuilding without allowing interaction",
  {
    skip:
      process.platform !== "darwin" ||
      process.env.ZHISHU_CCS_RUN_KEYCHAIN_SMOKE !== "1",
  },
  (t) => {
    const dir = fixture(t);
    const service = `com.zhishu.ccswitch.dev-signing-test.${randomUUID()}`;
    const env = {
      ...process.env,
      MACOS_DEV_SIGNING_IDENTITY: resolveIdentity().hash,
    };
    const versions = [path.join(dir, "probe-v1"), path.join(dir, "probe-v2")];
    const run = (binary, args) => {
      const result = spawnSync(binary, args, { encoding: "utf8" });
      assert.equal(
        result.status,
        0,
        `${binary}: ${result.stderr || result.stdout}`,
      );
      return `${result.stdout || ""}${result.stderr || ""}`;
    };
    const signatures = versions.map((executable, i) => {
      run("/usr/bin/cc", [
        path.join(root, "scripts/fixtures/macos-keychain-probe.c"),
        `-DPROBE_VERSION=${i + 1}`,
        "-Wno-deprecated-declarations",
        "-framework",
        "Security",
        "-framework",
        "CoreFoundation",
        "-o",
        executable,
      ]);
      signExecutable(executable, env);
      const output = run("/usr/bin/codesign", [
        "-d",
        "-r-",
        "--verbose=4",
        executable,
      ]);
      const cdhash = output.match(/^CDHash=(.+)$/m)?.[1];
      const requirement = output.match(/^designated => (.+)$/m)?.[1];
      assert.ok(cdhash, "codesign must report the executable hash");
      assert.ok(requirement, "codesign must report the designated requirement");
      return { cdhash, requirement };
    });
    assert.notEqual(signatures[0].cdhash, signatures[1].cdhash);
    assert.equal(signatures[0].requirement, signatures[1].requirement);
    run(versions[0], ["write", service]);
    try {
      assert.match(run(versions[1], ["read", service]), /version=2 status=0/);
      t.diagnostic(
        "Different CDHashes, identical Apple designated requirements; Keychain read succeeded with UI disabled.",
      );
    } finally {
      run(versions[0], ["delete", service]);
    }
  },
);
