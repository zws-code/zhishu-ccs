import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { readReleaseConfig } from "./release-config.mjs";

export function prepareRelease(
  root = new URL("../", import.meta.url),
  env = process.env,
) {
  const editable = env.ZHISHU_EDITABLE_BUILD === "true";
  const unsigned = env.ZHISHU_UNSIGNED_BUILD === "true" || editable;
  const config = readReleaseConfig(root, env, {
    requireUpdater: !unsigned,
  });
  writeIfChanged(
    new URL("src/.env.production.local", root),
    [
      `VITE_PLATFORM_API_URL=${config.platformApiUrl}`,
      `VITE_PLATFORM_WEB_URL=${config.platformWebUrl}`,
      `VITE_ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP=${config.platformAllowInsecureHttp}`,
      `VITE_ZHISHU_UPDATES_ENABLED=${!unsigned}`,
      "",
    ].join("\n"),
  );
  writeIfChanged(
    new URL("src-tauri/tauri.release.local.conf.json", root),
    JSON.stringify(
      {
        bundle: { createUpdaterArtifacts: !unsigned },
        plugins: {
          updater: {
            endpoints: config.updateUrl ? [config.updateUrl] : [],
            pubkey: config.updatePublicKey || "",
            ...(config.updateAllowInsecureHttp
              ? { dangerousInsecureTransportProtocol: true }
              : {}),
          },
        },
      },
      null,
      2,
    ) + "\n",
  );
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    prepareRelease();
    console.log(
      "Zhishu production configuration prepared. No signing private key is written by this script.",
    );
  } catch (error) {
    console.error(`Release configuration: ${error.message}`);
    process.exitCode = 1;
  }
}

function writeIfChanged(path, content) {
  try {
    if (readFileSync(path, "utf8") === content) return;
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  writeFileSync(path, content);
}
