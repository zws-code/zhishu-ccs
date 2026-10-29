#!/usr/bin/env python3
"""Publish a verified CI payload to COS. Imports cloud SDK only for actual uploads."""
import argparse
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import logging
import os
from pathlib import Path
import re
import sys
import time
import traceback
import threading
from urllib.request import Request, urlopen
from urllib.parse import urlsplit


class ReleaseError(Exception):
    pass


def credential(env, name, optional=False):
    value = env.get(name, "").strip()
    if not value and not optional:
        raise ReleaseError("Missing environment variable: " + name)
    if any(character.isspace() for character in value):
        raise ReleaseError(name + " contains whitespace inside the credential; re-enter this GitHub Secret")
    return value or None


def safe_error(error):
    if isinstance(error, ReleaseError):
        return str(error)
    kind = type(error).__name__
    if kind == "CosClientError":
        message = str(error).lower()
        categories = [
            (("byte budget exceeded",), "accelerated upload byte budget exceeded; stop before retrying"),
            (("secret_id", "secretkey", "secretid", "secret_key"), "credential format; check GitHub COS Secrets"),
            (("ssl", "certificate", "tls"), "TLS connection; check runner-to-COS connectivity"),
            (("proxy",), "proxy connection; check runner network"),
            (("timeout", "timed out"), "network timeout; retry with the same payload"),
            (("connection", "name resolution", "getaddrinfo"), "network connection; retry with the same payload"),
        ]
        for words, category in categories:
            if any(word in message for word in words):
                return "COS client error: " + category
    if kind == "CosServiceError":
        code = str(error.get_error_code())
        status = str(error.get_status_code())
        if re.fullmatch(r"[A-Za-z][A-Za-z0-9]{0,63}", code) and re.fullmatch(r"[1-5][0-9]{2}", status):
            return "COS service error: HTTP " + status + " / " + code
    cause = error.__cause__ or error.__context__
    if cause is not None:
        kind += "/" + type(cause).__name__
    frames = [
        Path(frame.filename).name + ":" + str(frame.lineno) + "/" + frame.name
        for frame in traceback.extract_tb(error.__traceback__)
        if Path(frame.filename).name in {"cos_client.py", "cos_comm.py", "cos_auth.py"}
    ]
    location = " at " + " -> ".join(frames[-3:]) if frames else ""
    return "COS publication failed (" + kind + ")" + location + "; check credentials, bucket permissions and network"


def version(value):
    if not isinstance(value, str) or not re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", value):
        raise ReleaseError("Invalid stable version")
    return tuple(map(int, value.split(".")))


def digest(path):
    # Ubuntu 22.04's system Python is 3.10; file_digest requires Python 3.11.
    checksum = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            checksum.update(chunk)
    return checksum.hexdigest()


def load_payload(directory):
    directory = Path(directory)
    receipt = json.loads((directory / "release.json").read_text())
    version(receipt["version"])
    if receipt["tag"] != "v" + receipt["version"]:
        raise ReleaseError("Receipt tag mismatch")
    names = set()
    for item in receipt["files"]:
        name = item["name"]
        if not re.fullmatch(r"[A-Za-z0-9._-]+", name) or name in names or name in (".", "..", "release.json"):
            raise ReleaseError("Invalid or duplicate payload name")
        names.add(name)
        path = directory / name
        if path.is_symlink() or not path.is_file() or not 0 < item["size"] <= 5 * 1024**3:
            raise ReleaseError("Invalid payload file")
        if path.stat().st_size != item["size"] or digest(path) != item["sha256"]:
            raise ReleaseError("Payload checksum mismatch: " + name)
    if not {"latest.json", "client-release.json"} <= names:
        raise ReleaseError("Missing release manifests")
    if {p.name for p in directory.iterdir()} != names | {"release.json"}:
        raise ReleaseError("Unexpected payload files")
    latest = json.loads((directory / "latest.json").read_text())
    website = json.loads((directory / "client-release.json").read_text())
    if latest["version"] != receipt["version"] or website["version"] != receipt["version"]:
        raise ReleaseError("Manifest version mismatch")
    expected = receipt["downloadBaseUrl"] + "/" + receipt["tag"] + "/"
    if not latest.get("platforms") or not website.get("files"):
        raise ReleaseError("Empty release manifests")
    for entry in [*latest["platforms"].values(), *website["files"]]:
        if not entry["url"].startswith(expected) or entry["url"][len(expected):] not in names:
            raise ReleaseError("Manifest references a missing or unexpected file")
    return receipt, latest


def check_promotion(current, incoming):
    if current is None:
        return
    if version(current["version"]) > version(incoming["version"]):
        raise ReleaseError("Refusing to downgrade the stable release")
    if not set(current["platforms"]) <= set(incoming["platforms"]):
        raise ReleaseError("Previously released platforms are missing")
    if current["version"] == incoming["version"] and current != incoming:
        raise ReleaseError("A published version is immutable; use a new version")


class UploadBudget:
    """Conservatively count all bytes read for accelerated PUTs, including retries."""
    def __init__(self, maximum):
        self.maximum, self.used = maximum, 0
        self.lock = threading.Lock()

    def consume(self, count):
        with self.lock:
            if self.used + count > self.maximum:
                raise ReleaseError("Accelerated upload byte budget exceeded; stop before retrying")
            self.used += count


class UploadBody:
    """Expose a seekable file and report reads by the HTTP uploader, never hash passes."""
    def __init__(self, stream, name, total, budget=None):
        self.stream, self.name, self.total = stream, name, total
        self.budget = budget
        self.reported = set()

    def __getattr__(self, name):
        return getattr(self.stream, name)

    def seek(self, offset, whence=0):
        before = self.stream.tell()
        result = self.stream.seek(offset, whence)
        if result < before:
            self.reported.clear()
        return result

    def read(self, size=-1):
        chunk = self.stream.read(size)
        if self.budget is not None:
            self.budget.consume(len(chunk))
        position = self.stream.tell()
        if chunk and self.total >= 1024 * 1024:
            for milestone in (1, 25, 50, 75, 100):
                threshold = min(1024 * 1024, self.total) if milestone == 1 else self.total * milestone // 100
                if position >= threshold and milestone not in self.reported:
                    self.reported.add(milestone)
                    message = "Upload stream " + self.name + ": " + str(position) + "/" + str(self.total) + " bytes"
                    print(message, flush=True)
                    if os.environ.get("GITHUB_ACTIONS") == "true":
                        print("::notice title=COS upload progress::" + message, flush=True)
        return chunk


class CosStore:
    def __init__(self, env):
        from qcloud_cos import CosConfig, CosS3Client
        self.bucket = env["COS_BUCKET"]
        accelerate = env.get("COS_UPLOAD_ACCELERATE", "false")
        if accelerate not in ("true", "false"):
            raise ReleaseError("COS_UPLOAD_ACCELERATE must be true or false")
        config = dict(
            Region=env["COS_REGION"], SecretId=credential(env, "COS_SECRET_ID"),
            SecretKey=credential(env, "COS_SECRET_KEY"), Token=credential(env, "COS_SESSION_TOKEN", optional=True),
            Scheme="https", Timeout=120,
        )
        self.client = CosS3Client(CosConfig(**config))
        self.upload_client = self.client
        self.upload_budget = None
        if accelerate == "true":
            # <= 3 GB at the current cross-border rate leaves margin under the
            # explicitly approved CNY 5 acceleration budget for this recovery.
            self.upload_budget = UploadBudget(3 * 1000**3)
            self.upload_client = CosS3Client(CosConfig(**config, Endpoint="cos.accelerate.myqcloud.com"))

    def head(self, key):
        from qcloud_cos.cos_exception import CosServiceError
        try:
            return self.client.head_object(Bucket=self.bucket, Key=key)
        except CosServiceError as error:
            # Do not mistake authorization/network failures for missing objects.
            if error.get_status_code() == 404 and error.get_error_code() in ("NoSuchKey", "NoSuchResource"):
                return None
            raise

    def read(self, key):
        if self.head(key) is None:
            return None
        stream = self.client.get_object(Bucket=self.bucket, Key=key)["Body"].get_raw_stream()
        try:
            return stream.read()
        finally:
            stream.close()

    def put(self, key, path, immutable):
        with path.open("rb") as stream:
            checksum = hashlib.md5()
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                checksum.update(chunk)
            stream.seek(0)
            body = UploadBody(stream, path.name, path.stat().st_size, getattr(self, "upload_budget", None))
            self.upload_client.put_object(
                Bucket=self.bucket, Key=key, Body=body, EnableMD5=False,
                ContentMD5=base64.b64encode(checksum.digest()).decode("ascii"),
                # SDK 1.9.44 has no ForbidOverwrite keyword. Its Metadata map
                # passes header names through unchanged, including custom COS headers.
                Metadata={
                    "x-cos-meta-sha256": digest(path),
                    "x-cos-forbid-overwrite": "true" if immutable else "false",
                },
                ContentType="application/json" if path.suffix == ".json" else "application/octet-stream",
                CacheControl="public, max-age=31536000, immutable" if immutable else "public, max-age=60",
            )

    def ensure(self, key, path):
        existing = self.head(key)
        if existing is None:
            self.put(key, path, True)
            existing = self.head(key)
        normalized = {k.lower(): v for k, v in existing.items()}
        if int(normalized.get("content-length", -1)) != path.stat().st_size or normalized.get("x-cos-meta-sha256") != digest(path):
            raise ReleaseError("An immutable object differs: " + key)


def public_check(url, path, manifest=False):
    request = Request(url, method="GET" if manifest else "HEAD", headers={"Cache-Control": "no-cache"})
    with urlopen(request, timeout=30) as response:
        if urlsplit(response.url).scheme != "https":
            raise ReleaseError("Public URL redirected away from HTTPS")
        if response.status != 200:
            raise ReleaseError("Public URL is not available")
        if manifest:
            if hashlib.sha256(response.read(2 * 1024**2)).hexdigest() != digest(path):
                raise ReleaseError("CDN manifest is stale or differs")
        elif int(response.headers.get("Content-Length", -1)) != path.stat().st_size:
            raise ReleaseError("Public artifact size mismatch")


def retry_public(check, url, path, manifest=False):
    for attempt in range(10):
        try:
            check(url, path, manifest)
            return
        except Exception:
            if attempt == 9:
                raise ReleaseError("CDN validation failed: " + url) from None
            time.sleep(10)


def publish(directory, store, prefix, base, check=public_check, wait=retry_public):
    directory = Path(directory)
    receipt, latest = load_payload(directory)
    if base != receipt["downloadBaseUrl"]:
        raise ReleaseError("Public download base differs from the verified payload")
    stable_key = prefix + "/latest.json"
    current_bytes = store.read(stable_key)
    check_promotion(json.loads(current_bytes) if current_bytes else None, latest)
    website_bytes = store.read(prefix + "/client-release.json")
    if website_bytes and version(json.loads(website_bytes)["version"]) > version(latest["version"]):
        raise ReleaseError("Website manifest already points to a newer version")
    # Reserve the exact inputs before any installer upload, so a failed upload can
    # resume but another build cannot replace this version's contents.
    version_prefix = prefix + "/" + receipt["tag"] + "/"
    print("Reserving immutable release inputs", flush=True)
    store.ensure(version_prefix + "release.json", directory / "release.json")
    def upload(item):
        print("Uploading/verifying " + item["name"], flush=True)
        store.ensure(version_prefix + item["name"], directory / item["name"])
        print("COS verified " + item["name"], flush=True)
    # Independent immutable objects may upload concurrently. Promotion still waits
    # for every upload and public check, and the version receipt is reserved first.
    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(upload, receipt["files"]))
    # All referenced files must be downloadable before changing either pointer.
    for item in receipt["files"]:
        wait(check, base + "/" + receipt["tag"] + "/" + item["name"], directory / item["name"], item["name"].endswith(".json"))
    # Concurrency in the workflow serializes all releases of this repository.
    # Recheck just before promotion as protection against out-of-order builds.
    current_bytes = store.read(stable_key)
    check_promotion(json.loads(current_bytes) if current_bytes else None, latest)
    for name in ("client-release.json", "latest.json"):
        store.put(prefix + "/" + name, directory / name, False)
        if store.read(prefix + "/" + name) != (directory / name).read_bytes():
            raise ReleaseError("COS manifest read-back mismatch")
        wait(check, base + "/" + name, directory / name, True)
    print("Published " + receipt["tag"] + " at " + base, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory")
    parser.add_argument("--dry-run", action="store_true", help="Validate local payload only; never contact COS/CDN")
    args = parser.parse_args()
    receipt, _ = load_payload(args.directory)
    if args.dry_run:
        print("Validated " + receipt["tag"] + "; no network requests or uploads")
        return
    for name in ("COS_BUCKET", "COS_REGION", "COS_SECRET_ID", "COS_SECRET_KEY", "COS_PUBLIC_BASE_URL"):
        if not os.environ.get(name):
            raise ReleaseError("Missing environment variable: " + name)
    prefix = os.environ.get("COS_PREFIX") or "ccs/stable"
    if not re.fullmatch(r"[A-Za-z0-9_-]+(?:/[A-Za-z0-9_-]+)*", prefix):
        raise ReleaseError("Invalid COS_PREFIX")
    base = os.environ["COS_PUBLIC_BASE_URL"].rstrip("/")
    url = urlsplit(base)
    if url.scheme != "https" or url.path != "/" + prefix or url.query or url.fragment or url.username:
        raise ReleaseError("Invalid COS_PUBLIC_BASE_URL")
    # COS forbid-overwrite does not protect version-enabled buckets. Explicitly
    # require versioning disabled; preserve historical versions via immutable paths.
    print("Initializing COS client", flush=True)
    store = CosStore(os.environ)
    print("Checking bucket versioning", flush=True)
    status = store.client.get_bucket_versioning(Bucket=store.bucket).get("Status")
    if status not in (None, ""):
        raise ReleaseError("Use a dedicated bucket with COS versioning disabled")
    publish(args.directory, store, prefix, base)


if __name__ == "__main__":
    logging.disable(logging.CRITICAL)  # SDK diagnostics may contain signed URLs.
    try:
        main()
    except Exception as error:
        # Never echo raw SDK exceptions/headers/credentials into Actions logs.
        message = safe_error(error)
        print(message, file=sys.stderr)
        if os.environ.get("GITHUB_ACTIONS") == "true":
            # Publish only the already-sanitized diagnostic as a check annotation.
            annotation = message.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
            print("::error::" + annotation, flush=True)
        sys.exit(1)
