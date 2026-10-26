#!/usr/bin/env python3
"""Atomic, immutable release publication on a Linux SSH host (stdlib only)."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile
from urllib.parse import unquote, urlsplit

VERSION = re.compile(r"^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
NAME = re.compile(r"^[A-Za-z0-9._-]+$")


def digest(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def validate(directory):
    data = json.loads((directory / "release.json").read_text())
    tag = data.get("tag", "")
    if not VERSION.fullmatch(tag) or data.get("version") != tag[1:]:
        raise ValueError("Invalid release version")
    listed = {}
    for file in data["files"]:
        name = file["name"]
        if not NAME.fullmatch(name) or name.startswith(".") or name in listed:
            raise ValueError("Invalid or duplicate release filename")
        path = directory / name
        if path.is_symlink() or not path.is_file() or path.stat().st_size != file["size"] or digest(path) != file["sha256"]:
            raise ValueError("Corrupt release asset: " + name)
        listed[name] = file
    expected = set(listed) | {"release.json", "SHA256SUMS"}
    if set(p.name for p in directory.iterdir()) != expected:
        raise ValueError("Unexpected files in release directory")
    sums = (directory / "SHA256SUMS").read_text().splitlines()
    expected_sums = {f'{file["sha256"]}  {name}' for name, file in listed.items()} | {digest(directory / "release.json") + "  release.json"}
    if len(sums) != len(expected_sums) or set(sums) != expected_sums:
        raise ValueError("Checksum manifest mismatch")
    latest = json.loads((directory / "latest.json").read_text())
    website = json.loads((directory / "client-release.json").read_text())
    if latest["version"] != data["version"] or website["version"] != data["version"] or not latest.get("platforms") or not website.get("files"):
        raise ValueError("Release manifest mismatch")
    for entry in list(latest["platforms"].values()) + website["files"]:
        url = urlsplit(entry["url"])
        parts = unquote(url.path).rstrip("/").split("/")
        if len(parts) < 3 or parts[-2] != tag or parts[-1] not in listed or url.scheme not in ("http", "https"):
            raise ValueError("Manifest references a missing asset")
    return data


def atomic_file(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temp = tempfile.mkstemp(prefix=".client-release-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(content)
            f.flush()
            os.fsync(f.fileno())
            os.fchmod(f.fileno(), 0o644)
        os.replace(temp, path)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)


def release(root, stage, proxy_manifest, activate):
    root, stage, proxy_manifest = Path(root).resolve(), Path(stage).resolve(), Path(proxy_manifest)
    if not (root in stage.parents and ".staging" in stage.parts) and not (activate and stage.parent == root / "versions"):
        raise ValueError("Staging directory must be inside the configured release root")
    root.mkdir(parents=True, exist_ok=True)
    with open(root / ".publish.lock", "a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        data = validate(stage)
        current = root / "current"
        previous = current.resolve() if current.exists() else None
        if previous:
            old = validate(previous)
            if tuple(map(int, old["version"].split("."))) > tuple(map(int, data["version"].split("."))):
                raise ValueError("Refusing to publish an older version")
            if not set(old["targets"]).issubset(data["targets"]):
                raise ValueError("Missing previously released targets; build/import all supported platforms first")
        destination = root / "versions" / data["tag"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        if destination.exists():
            validate(destination)
            if (destination / "SHA256SUMS").read_bytes() != (stage / "SHA256SUMS").read_bytes():
                raise ValueError("Published versions are immutable; use a new version number")
        else:
            # Staging lives on the same filesystem. Every file was checked before rename.
            for path in stage.iterdir():
                path.chmod(0o644)
            stage.chmod(0o755)
            os.rename(stage, destination)
        if not activate:
            print(json.dumps({"prepared": data["tag"]}))
            return
        payload = (destination / "client-release.json").read_bytes()
        old_proxy = proxy_manifest.read_bytes() if proxy_manifest.exists() else None
        if proxy_manifest.exists() and not (root / "original-client-release.json").exists():
            atomic_file(root / "original-client-release.json", old_proxy)
        # Preflight the platform override directory before changing the updater pointer.
        proxy_manifest.parent.mkdir(parents=True, exist_ok=True)
        fd, probe = tempfile.mkstemp(prefix=".publish-probe-", dir=proxy_manifest.parent)
        os.close(fd)
        os.unlink(probe)
        pending = root / ".current-next"
        if pending.is_symlink():
            pending.unlink()
        pending.symlink_to(Path("versions") / data["tag"])
        os.replace(pending, current)
        try:
            atomic_file(proxy_manifest, payload)
        except Exception:
            if previous:
                pending.symlink_to(previous)
                os.replace(pending, current)
            else:
                current.unlink()
            if old_proxy is not None:
                atomic_file(proxy_manifest, old_proxy)
            raise
        print(json.dumps({"published": data["tag"], "targets": data["targets"]}))


if __name__ == "__main__":
    try:
        if sys.argv[1] in ("prepare", "publish"):
            release(*sys.argv[2:5], activate=sys.argv[1] == "publish")
        else:
            raise ValueError("Expected prepare/publish")
    except Exception as exc:
        print("Release error: " + str(exc), file=sys.stderr)
        sys.exit(1)
