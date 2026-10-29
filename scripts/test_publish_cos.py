import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
from unittest.mock import patch
import unittest

spec = importlib.util.spec_from_file_location("publish_cos", Path(__file__).with_name("publish-cos.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

BASE = "https://download.zhishu.cn/ccs/stable"
PREFIX = "ccs/stable"


class MemoryStore:
    def __init__(self):
        self.objects = {}
        self.events = []
        self.fail_at = None

    def read(self, key):
        return self.objects.get(key)

    def ensure(self, key, path):
        data = path.read_bytes()
        if key in self.objects and self.objects[key] != data:
            raise module.ReleaseError("immutable collision")
        self.put(key, path, True)

    def put(self, key, path, immutable):
        self.events.append((key, immutable))
        if key == self.fail_at:
            raise module.ReleaseError("simulated upload failure")
        self.objects[key] = path.read_bytes()


def payload(directory, number="1.0.0", platforms=None):
    platforms = platforms or ["darwin-aarch64", "darwin-x86_64", "windows-x86_64"]
    installer = directory / "test.msi"
    installer.write_bytes(b"signed-fixture-installer")
    url = BASE + "/v" + number + "/test.msi"
    latest = {"version": number, "platforms": {name: {"url": url, "signature": "fixture"} for name in platforms}}
    (directory / "latest.json").write_text(json.dumps(latest))
    (directory / "client-release.json").write_text(json.dumps({"version": number, "files": [{"url": url}]}))
    files = [{"name": path.name, "size": path.stat().st_size, "sha256": module.digest(path)} for path in sorted(directory.iterdir())]
    (directory / "release.json").write_text(json.dumps({"version": number, "tag": "v" + number, "downloadBaseUrl": BASE, "files": files}))
    return latest


class PublisherTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="zhishu-cos-publish-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.latest = payload(self.directory)
        self.store = MemoryStore()
        self.checks = []

    def check(self, url, path, manifest=False):
        self.checks.append(url)
        key = PREFIX + url.removeprefix(BASE)
        self.assertEqual(self.store.objects[key], path.read_bytes())
        if url == BASE + "/latest.json":
            self.assertIn(PREFIX + "/client-release.json", self.store.objects)

    def publish(self, check=None):
        module.publish(self.directory, self.store, PREFIX, BASE, check or self.check, lambda fn, *args: fn(*args))

    def test_artifacts_verified_before_stable_manifests_and_safe_retry(self):
        self.publish()
        self.assertEqual([key for key, immutable in self.store.events if not immutable], [PREFIX + "/client-release.json", PREFIX + "/latest.json"])
        self.assertEqual(json.loads(self.store.objects[PREFIX + "/latest.json"]), self.latest)
        before = dict(self.store.objects)
        self.publish()
        self.assertEqual(self.store.objects, before)

    def test_upload_failure_leaves_both_stable_manifests_untouched(self):
        self.store.fail_at = PREFIX + "/v1.0.0/test.msi"
        with self.assertRaises(module.ReleaseError):
            self.publish()
        self.assertNotIn(PREFIX + "/latest.json", self.store.objects)
        self.assertNotIn(PREFIX + "/client-release.json", self.store.objects)
        self.store.fail_at = None
        self.publish()

    def test_public_download_failure_never_promotes(self):
        def fail(*args):
            raise module.ReleaseError("CDN unavailable")
        with self.assertRaises(module.ReleaseError):
            self.publish(fail)
        self.assertNotIn(PREFIX + "/latest.json", self.store.objects)
        self.assertNotIn(PREFIX + "/client-release.json", self.store.objects)

    def test_newer_stable_or_missing_platforms_refused_before_upload(self):
        for current in [
            {**self.latest, "version": "2.0.0"},
            {"version": "0.9.0", "platforms": {"linux-aarch64": {}}},
            {**self.latest, "notes": "different release"},
        ]:
            self.store.objects[PREFIX + "/latest.json"] = json.dumps(current).encode()
            with self.assertRaises(module.ReleaseError):
                self.publish()
            self.assertEqual(self.store.events, [])

    def test_recheck_rejects_newer_release_appearing_during_upload(self):
        def promote_during_check(*args):
            self.store.objects[PREFIX + "/latest.json"] = json.dumps({**self.latest, "version": "2.0.0"}).encode()
        with self.assertRaises(module.ReleaseError):
            self.publish(promote_during_check)
        self.assertEqual(json.loads(self.store.objects[PREFIX + "/latest.json"])["version"], "2.0.0")
        self.assertFalse(any(not immutable for _, immutable in self.store.events))

    def test_reserved_version_cannot_be_replaced_with_other_inputs(self):
        self.store.objects[PREFIX + "/v1.0.0/release.json"] = b"other build"
        with self.assertRaises(module.ReleaseError):
            self.publish()
        self.assertNotIn(PREFIX + "/v1.0.0/test.msi", self.store.objects)

    def test_failure_between_manifest_writes_can_resume(self):
        self.store.fail_at = PREFIX + "/latest.json"
        with self.assertRaises(module.ReleaseError):
            self.publish()
        self.assertIn(PREFIX + "/client-release.json", self.store.objects)
        self.assertNotIn(PREFIX + "/latest.json", self.store.objects)
        self.store.fail_at = None
        self.publish()

    def test_corrupt_payload_and_unlisted_files_are_rejected(self):
        (self.directory / "unlisted.key").write_bytes(b"secret")
        with self.assertRaises(module.ReleaseError):
            self.publish()
        self.assertEqual(self.store.events, [])
        (self.directory / "unlisted.key").unlink()
        (self.directory / "test.msi").write_bytes(b"corrupt")
        with self.assertRaises(module.ReleaseError):
            self.publish()

    def test_dry_run_needs_no_credentials_or_sdk_and_uploads_nothing(self):
        result = subprocess.run([sys.executable, "-B", str(Path(module.__file__)), str(self.directory), "--dry-run"], capture_output=True, text=True, env={})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("no network requests or uploads", result.stdout)


class HashCompatibilityTests(unittest.TestCase):
    def test_hashes_across_chunks_without_python_311_file_digest(self):
        data = b"release-payload" * 100000
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "installer"
            path.write_bytes(data)
            old_hashlib = types.SimpleNamespace(sha256=hashlib.sha256)
            with patch.object(module, "hashlib", old_hashlib):
                self.assertEqual(module.digest(path), hashlib.sha256(data).hexdigest())


class CredentialAndDiagnosticTests(unittest.TestCase):
    def test_copy_whitespace_is_trimmed_without_echoing_secrets(self):
        self.assertEqual(module.credential({"COS_SECRET_ID": "  example-id\n"}, "COS_SECRET_ID"), "example-id")
        self.assertEqual(module.credential({"COS_SECRET_KEY": "\nexample-key\r\n"}, "COS_SECRET_KEY"), "example-key")
        self.assertIsNone(module.credential({}, "COS_SESSION_TOKEN", optional=True))
        for value in ("", " ", "example key"):
            with self.assertRaises(module.ReleaseError) as caught:
                module.credential({"COS_SECRET_KEY": value}, "COS_SECRET_KEY")
            self.assertNotIn("example key", str(caught.exception))

    def test_safe_diagnostics_do_not_echo_signed_urls_or_credentials(self):
        ClientError = type("CosClientError", (Exception,), {})
        secret = "SECRET-DO-NOT-LOG"
        for message, expected in [
            ("secret_id cannot contain spaces " + secret, "credential format"),
            ("SSL error https://cos/?q-signature=" + secret, "TLS connection"),
            ("Read timed out " + secret, "network timeout"),
            ("ConnectionError " + secret, "network connection"),
            ("ProxyError " + secret, "proxy connection"),
            ("unknown " + secret, "CosClientError"),
        ]:
            result = module.safe_error(ClientError(message))
            self.assertIn(expected, result)
            self.assertNotIn(secret, result)
        wrapped = ClientError("opaque " + secret)
        wrapped.__context__ = TypeError("private header " + secret)
        self.assertIn("CosClientError/TypeError", module.safe_error(wrapped))
        self.assertNotIn(secret, module.safe_error(wrapped))
        ServiceError = type("CosServiceError", (Exception,), {
            "get_error_code": lambda _: "AccessDenied",
            "get_status_code": lambda _: 403,
        })
        self.assertEqual(module.safe_error(ServiceError(secret)), "COS service error: HTTP 403 / AccessDenied")


class CosAdapterTests(unittest.TestCase):
    def test_head_404_sdk_code_is_absent_but_permission_errors_are_not(self):
        class ServiceError(Exception):
            def __init__(self, status, code):
                self.status, self.code = status, code
            def get_status_code(self):
                return self.status
            def get_error_code(self):
                return self.code
        store = module.CosStore.__new__(module.CosStore)
        store.bucket = "test-123"
        exception_module = types.ModuleType("qcloud_cos.cos_exception")
        exception_module.CosServiceError = ServiceError
        with patch.dict(sys.modules, {"qcloud_cos.cos_exception": exception_module}):
            for code in ("NoSuchResource", "NoSuchKey"):
                store.client = types.SimpleNamespace(head_object=lambda **kwargs: (_ for _ in ()).throw(ServiceError(404, code)))
                self.assertIsNone(store.head("ccs/stable/latest.json"))
            for status, code in ((403, "AccessDenied"), (404, "NoSuchBucket"), (500, "InternalError")):
                store.client = types.SimpleNamespace(head_object=lambda **kwargs: (_ for _ in ()).throw(ServiceError(status, code)))
                with self.assertRaises(ServiceError):
                    store.head("ccs/stable/latest.json")

    def test_upload_passes_cos_metadata_header_and_forbid_overwrite(self):
        captured = []
        store = module.CosStore.__new__(module.CosStore)
        store.bucket = "test-123"
        store.client = types.SimpleNamespace(put_object=lambda **kwargs: captured.append(kwargs))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test.msi"
            path.write_bytes(b"installer")
            store.put("ccs/stable/v1.0.0/test.msi", path, True)
            self.assertEqual(captured[-1]["Metadata"], {"x-cos-meta-sha256": module.digest(path)})
            self.assertEqual(captured[-1]["ForbidOverwrite"], "true")
            self.assertTrue(captured[-1]["EnableMD5"])
            store.put("ccs/stable/latest.json", path, False)
            self.assertEqual(captured[-1]["ForbidOverwrite"], "false")
            self.assertEqual(captured[-1]["CacheControl"], "public, max-age=60")


if __name__ == "__main__":
    unittest.main()
