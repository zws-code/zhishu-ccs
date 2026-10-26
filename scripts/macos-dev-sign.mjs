#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const identityTypes = [
  "Apple Development:",
  "Mac Developer:",
  "Developer ID Application:",
  "Apple Distribution:",
  "3rd Party Mac Developer Application:",
];
const setupHint = "See docs/macos-dev-signing.md for certificate setup.";

export function selectIdentity(output, requested = "") {
  const identities = [
    ...output.matchAll(/^\s*\d+\) ([A-Fa-f0-9]{40}) "([^"]+)"\s*$/gm),
  ]
    .map((match) => ({ hash: match[1].toUpperCase(), name: match[2] }))
    .filter(({ name }) =>
      identityTypes.some((prefix) => name.startsWith(prefix)),
    );
  const unique = [
    ...new Map(identities.map((entry) => [entry.hash, entry])).values(),
  ];
  let matches;
  if (requested) {
    matches = unique.filter(
      ({ hash, name }) =>
        hash === requested.toUpperCase() || name === requested,
    );
  } else {
    const preferred = identityTypes.find((prefix) =>
      unique.some(({ name }) => name.startsWith(prefix)),
    );
    matches = preferred
      ? unique.filter(({ name }) => name.startsWith(preferred))
      : [];
  }
  if (matches.length === 1) return matches[0];
  if (matches.length > 1) {
    throw new Error(
      `Multiple signing identities match. Set MACOS_DEV_SIGNING_IDENTITY to one SHA-1 fingerprint:\n${matches.map(({ hash, name }) => `${hash} ${name}`).join("\n")}`,
    );
  }
  throw new Error(
    `No matching valid Apple-issued signing identity. Self-signed and ad-hoc signatures still cause Keychain prompts after rebuilds. ${setupHint}`,
  );
}

function command(run, binary, args) {
  const result = run(binary, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.error || result.status !== 0) {
    throw new Error(
      `${path.basename(binary)} failed: ${result.error?.message || result.stderr?.trim() || `exit ${result.status}`}`,
    );
  }
  return `${result.stdout || ""}${result.stderr || ""}`;
}

export function resolveIdentity(env = process.env, run = spawnSync) {
  return selectIdentity(
    command(run, "/usr/bin/security", [
      "find-identity",
      "-v",
      "-p",
      "codesigning",
    ]),
    env.MACOS_DEV_SIGNING_IDENTITY,
  );
}

export function signExecutable(executable, env = process.env, run = spawnSync) {
  const identity = resolveIdentity(env, run);
  const identifier =
    env.MACOS_DEV_SIGNING_IDENTIFIER || "com.zhishu.ccswitch.desktop";
  command(run, "/usr/bin/codesign", [
    "--force",
    "--sign",
    identity.hash,
    "--identifier",
    identifier,
    "--timestamp=none",
    executable,
  ]);
  // A matching certificate name alone cannot prove Apple trust or a team partition.
  command(run, "/usr/bin/codesign", [
    "--verify",
    "--strict",
    "-R=anchor apple generic",
    executable,
  ]);
  const details = command(run, "/usr/bin/codesign", [
    "-d",
    "--verbose=4",
    executable,
  ]);
  const team = details.match(/^TeamIdentifier=([A-Z0-9]{10})$/m)?.[1];
  if (!team || !details.split("\n").includes(`Identifier=${identifier}`)) {
    throw new Error(
      "Signed executable has no stable Apple team identity or the wrong identifier; refusing to launch.",
    );
  }
  return { ...identity, team };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    if (process.platform !== "darwin")
      throw new Error("macOS development signing requires macOS.");
    if (process.argv[2] === "--identity") {
      console.log(resolveIdentity().hash);
    } else {
      if (!process.argv[2]) throw new Error("Missing executable path.");
      const identity = signExecutable(process.argv[2]);
      console.error(
        `[DEV] Signed with ${identity.name}; Keychain team ${identity.team}`,
      );
    }
  } catch (error) {
    console.error(`[DEV] ${error.message}`);
    process.exitCode = 1;
  }
}
