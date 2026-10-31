import { readFileSync } from "node:fs";

function isLoopback(hostname) {
  return (
    hostname === "localhost" ||
    hostname === "[::1]" ||
    hostname === "::1" ||
    /^127\./.test(hostname)
  );
}

export function productionUrl(
  value,
  name,
  { originOnly = false, allowInsecureHttp = false } = {},
) {
  const expectedProtocols = allowInsecureHttp ? "HTTP or HTTPS" : "HTTPS";
  if (
    typeof value !== "string" ||
    !value ||
    value !== value.trim() ||
    /[\s\\]/.test(value)
  )
    throw new Error(
      `${name} must be configured with an ${expectedProtocols} URL`,
    );
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(
      `${name} must be configured with an ${expectedProtocols} URL`,
    );
  }
  const protocolAllowed =
    url.protocol === "https:" ||
    (allowInsecureHttp && url.protocol === "http:");
  const hasHost = url.hostname.includes(".") || url.hostname.startsWith("[");
  if (
    !protocolAllowed ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    !hasHost ||
    /(^|\.)(localhost|local|invalid|example|test)$/.test(url.hostname) ||
    /(^|\.)example\.(com|org|net)$/.test(url.hostname) ||
    (url.protocol === "http:" && isLoopback(url.hostname)) ||
    (originOnly && url.pathname !== "/")
  )
    throw new Error(
      `${name} must use a production ${expectedProtocols} ${originOnly ? "origin" : "URL"} without credentials, query, fragment, or loopback host`,
    );
  if (
    /(^|\.)ccswitch\.io$/i.test(url.hostname) ||
    (url.hostname === "github.com" &&
      /^\/farion1231\/cc-switch(?:\/|$)/i.test(url.pathname))
  )
    throw new Error(`${name} must use your own release service`);
  return originOnly ? url.origin : url.href;
}

export function parseUpdatePublicKey(value) {
  if (typeof value !== "string" || !/^[A-Za-z0-9+/]+={0,2}$/.test(value.trim()))
    throw new Error("updatePublicKey must be a Tauri minisign public key");
  const decoded = Buffer.from(value.trim(), "base64")
    .toString("utf8")
    .trim()
    .split(/\r?\n/);
  const bytes = decoded[1]
    ? Buffer.from(decoded[1], "base64")
    : Buffer.alloc(0);
  if (
    !decoded[0]?.startsWith("untrusted comment: minisign public key:") ||
    bytes.length !== 42 ||
    !["Ed", "ED"].includes(bytes.subarray(0, 2).toString())
  )
    throw new Error("updatePublicKey must be a Tauri minisign public key");
  const upstreamKey =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEM4MDI4QzlBNTczOTI4RTMKUldUaktEbFhtb3dDeUM5US9kT0FmdGR5Ti9vQzcwa2dTMlpibDVDUmQ2M0VGTzVOWnd0SGpFVlEK";
  const upstreamBytes = Buffer.from(
    Buffer.from(upstreamKey, "base64")
      .toString("utf8")
      .trim()
      .split(/\r?\n/)[1],
    "base64",
  );
  if (bytes.equals(upstreamBytes))
    throw new Error("Use your own update signing key");
  return bytes;
}

export function validateReleaseConfig(input) {
  const platformAllowInsecureHttp = input.platformAllowInsecureHttp === true;
  const api = productionUrl(input.platformApiUrl, "platformApiUrl", {
    originOnly: true,
    allowInsecureHttp: platformAllowInsecureHttp,
  });
  const web = productionUrl(input.platformWebUrl, "platformWebUrl", {
    originOnly: true,
    allowInsecureHttp: platformAllowInsecureHttp,
  });
  const updateAllowInsecureHttp = input.updateAllowInsecureHttp === true;
  const updateUrl = productionUrl(input.updateUrl, "updateUrl", {
    allowInsecureHttp: updateAllowInsecureHttp,
  });
  const pubkey =
    typeof input.updatePublicKey === "string"
      ? input.updatePublicKey.trim()
      : input.updatePublicKey;
  parseUpdatePublicKey(pubkey);

  return {
    platformApiUrl: api,
    platformWebUrl: web,
    platformAllowInsecureHttp,
    updateUrl,
    updatePublicKey: pubkey,
    updateAllowInsecureHttp,
  };
}

export function readReleaseConfig(root, env = process.env) {
  let file = {};
  try {
    file = JSON.parse(
      readFileSync(new URL("release.config.local.json", root), "utf8"),
    );
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const insecureEnv = env.ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP;
  return validateReleaseConfig(
    {
      platformApiUrl: env.VITE_PLATFORM_API_URL || file.platformApiUrl,
      platformWebUrl: env.VITE_PLATFORM_WEB_URL || file.platformWebUrl,
      platformAllowInsecureHttp:
        insecureEnv === undefined
          ? file.platformAllowInsecureHttp
          : parseBoolean(insecureEnv, "ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP"),
      updateUrl: env.ZHISHU_UPDATE_URL || file.updateUrl,
      updatePublicKey: env.ZHISHU_UPDATE_PUBLIC_KEY || file.updatePublicKey,
      updateAllowInsecureHttp:
        env.ZHISHU_UPDATE_ALLOW_INSECURE_HTTP === undefined
          ? file.updateAllowInsecureHttp
          : parseBoolean(
              env.ZHISHU_UPDATE_ALLOW_INSECURE_HTTP,
              "ZHISHU_UPDATE_ALLOW_INSECURE_HTTP",
            ),
    },
  );
}

function parseBoolean(value, name) {
  if (value !== "true" && value !== "false")
    throw new Error(`${name} must be true or false`);
  return value === "true";
}
