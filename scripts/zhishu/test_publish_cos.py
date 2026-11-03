import hashlib
import base64
from contextlib import redirect_stdout
import io
import threading
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
    platforms = platforms or ["darwin-aarch64", "darwin-x86_64", "windows-x86_64", "windows-aarch64"]
    installer = directory / "test.exe"
    installer.write_bytes(b"signed-fixture-installer")
    url = BASE + "/v" + number + "/test.exe"
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
        self.store.fail_at = PREFIX + "/v1.0.0/test.exe"
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
            {"version": "0.9.0", "platforms": {"windows-i686": {}}},
            {**self.latest, "notes": "different release"},
        ]:
            self.store.objects[PREFIX + "/latest.json"] = json.dumps(current).encode()
            with self.assertRaises(module.ReleaseError):
                self.publish()
            self.assertEqual(self.store.events, [])

    def test_linux_pause_promotes_without_deleting_historical_artifacts(self):
        current = {**self.latest, "version": "0.9.0", "platforms": {
            **self.latest["platforms"],
            **{key: {} for key in module.PAUSED_UPDATER_PLATFORMS},
        }}
        self.store.objects[PREFIX + "/latest.json"] = json.dumps(current).encode()
        old_key = PREFIX + "/v0.9.0/Linux.AppImage"
        self.store.objects[old_key] = b"old installer"
        self.publish()
        self.assertEqual(json.loads(self.store.objects[PREFIX + "/latest.json"]), self.latest)
        self.assertEqual(self.store.objects[old_key], b"old installer")

    def test_pause_exception_does_not_allow_other_platform_loss_or_same_version_changes(self):
        current = {**self.latest, "version": "0.9.0"}
        for key in [*self.latest["platforms"], "linux-riscv64"]:
            old = {**current, "platforms": {**current["platforms"], key: {}}}
            incoming = {**self.latest, "platforms": {k: v for k, v in self.latest["platforms"].items() if k != key}}
            with self.subTest(platform=key), self.assertRaisesRegex(module.ReleaseError, "platforms are missing"):
                module.check_promotion(old, incoming)
        old = {**self.latest, "platforms": {**self.latest["platforms"], "linux-aarch64": {}}}
        with self.assertRaisesRegex(module.ReleaseError, "immutable"):
            module.check_promotion(old, self.latest)

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
        self.assertNotIn(PREFIX + "/v1.0.0/test.exe", self.store.objects)

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
        (self.directory / "test.exe").write_bytes(b"corrupt")
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
        store.upload_client = store.client
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test.exe"
            path.write_bytes(b"installer")
            store.put("ccs/stable/v1.0.0/test.exe", path, True)
            self.assertEqual(captured[-1]["Metadata"], {"x-cos-meta-sha256": module.digest(path), "x-cos-forbid-overwrite": "true"})
            self.assertNotIn("ForbidOverwrite", captured[-1])
            self.assertFalse(captured[-1]["EnableMD5"])
            self.assertEqual(captured[-1]["ContentMD5"], base64.b64encode(hashlib.md5(b"installer").digest()).decode("ascii"))
            store.put("ccs/stable/latest.json", path, False)
            self.assertEqual(captured[-1]["Metadata"]["x-cos-forbid-overwrite"], "false")
            self.assertEqual(captured[-1]["CacheControl"], "public, max-age=60")


class AccelerationConfigTests(unittest.TestCase):
    def test_acceleration_is_explicit_and_only_changes_upload_endpoint(self):
        sdk = types.ModuleType("qcloud_cos")
        sdk.CosConfig = lambda **kwargs: kwargs
        sdk.CosS3Client = lambda config: types.SimpleNamespace(config=config)
        env = {"COS_BUCKET": "test-123", "COS_REGION": "ap-guangzhou", "COS_SECRET_ID": "id", "COS_SECRET_KEY": "key"}
        with patch.dict(sys.modules, {"qcloud_cos": sdk}):
            normal = module.CosStore(env)
            self.assertIs(normal.client, normal.upload_client)
            accelerated = module.CosStore({**env, "COS_UPLOAD_ACCELERATE": "true"})
            self.assertNotIn("Endpoint", accelerated.client.config)
            self.assertEqual(accelerated.upload_client.config["Endpoint"], "cos.accelerate.myqcloud.com")
            self.assertEqual(accelerated.upload_client.config["Scheme"], "https")
            with self.assertRaises(module.ReleaseError):
                module.CosStore({**env, "COS_UPLOAD_ACCELERATE": "yes"})


class UploadBudgetTests(unittest.TestCase):
    def test_retry_reads_count_again_and_budget_stops_before_handing_off_bytes(self):
        budget = module.UploadBudget(10)
        body = module.UploadBody(io.BytesIO(b"12345678"), "test", 8, budget)
        self.assertEqual(body.read(6), b"123456")
        body.seek(0)
        self.assertEqual(body.read(4), b"1234")
        with self.assertRaisesRegex(module.ReleaseError, "budget exceeded"):
            body.read(1)
        self.assertEqual(budget.used, 10)

    def test_shared_budget_cannot_overrun_when_workers_read_concurrently(self):
        budget = module.UploadBudget(7)
        with module.ThreadPoolExecutor(max_workers=4) as pool:
            futures = [pool.submit(budget.consume, 3) for _ in range(4)]
        errors = sum(isinstance(f.exception(), module.ReleaseError) for f in futures)
        self.assertEqual(errors, 2)
        self.assertEqual(budget.used, 6)


class UploadProgressTests(unittest.TestCase):
    def test_progress_wrap_preserves_bytes_and_sdk_retry_seek(self):
        data = b"installer" * 300000
        stream = io.BytesIO(data)
        body = module.UploadBody(stream, "test.exe", len(data))
        output = io.StringIO()
        with redirect_stdout(output):
            self.assertEqual(body.read(1024 * 1024), data[:1024 * 1024])
            body.seek(0)
            self.assertEqual(body.read(), data)
        self.assertIn("Upload stream test.exe", output.getvalue())
        self.assertEqual(body.tell(), len(data))

    def test_concurrent_uploads_finish_before_any_stable_pointer(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            payload(path)
            store = MemoryStore()
            original = store.ensure
            workers = set()
            gate = threading.Barrier(3)
            def ensure(key, source):
                if key.endswith("/release.json"):
                    return original(key, source)
                workers.add(threading.get_ident())
                gate.wait(timeout=5)
                self.assertNotIn(PREFIX + "/latest.json", store.objects)
                original(key, source)
            store.ensure = ensure
            def check(url, source, manifest=False):
                self.assertEqual(store.objects[PREFIX + url.removeprefix(BASE)], source.read_bytes())
            module.publish(path, store, PREFIX, BASE, check, lambda fn, *args: fn(*args))
            self.assertEqual(len(workers), 3)
            self.assertEqual([key for key, immutable in store.events if not immutable], [PREFIX + "/client-release.json", PREFIX + "/latest.json"])


class CosSdkContractTests(unittest.TestCase):
    def test_real_sdk_separates_accelerated_uploads_from_origin_bucket_api(self):
        try:
            import qcloud_cos
        except ImportError:
            self.skipTest("COS SDK is checked separately after installation in publication CI")
        store = module.CosStore({
            "COS_BUCKET": "sdk-contract-1234567890", "COS_REGION": "ap-guangzhou",
            "COS_SECRET_ID": "test-id", "COS_SECRET_KEY": "test-key",
            "COS_UPLOAD_ACCELERATE": "true",
        })
        self.assertEqual(store.client._conf.uri(bucket=store.bucket), "https://sdk-contract-1234567890.cos.ap-guangzhou.myqcloud.com/")
        self.assertEqual(store.upload_client._conf.uri(bucket=store.bucket), "https://sdk-contract-1234567890.cos.accelerate.myqcloud.com/")
        self.assertEqual(store.upload_budget.maximum, 3 * 1000**3)

    def test_real_sdk_serializes_immutable_header_and_md5_without_network(self):
        try:
            import qcloud_cos
        except ImportError:
            self.skipTest("COS SDK is checked separately after installation in publication CI")
        store = module.CosStore({
            "COS_BUCKET": "sdk-contract-1234567890", "COS_REGION": "ap-guangzhou",
            "COS_SECRET_ID": "test-id", "COS_SECRET_KEY": "test-key",
        })
        requests = []
        def capture(**kwargs):
            requests.append(kwargs)
            return types.SimpleNamespace(headers={"ETag": "test"})
        store.client.send_request = capture
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test.exe"
            path.write_bytes(b"actual SDK contract")
            for immutable in (True, False):
                store.put("ccs/stable/v1.0.0/test.exe", path, immutable)
                headers = requests[-1]["headers"]
                self.assertEqual(headers["x-cos-forbid-overwrite"], str(immutable).lower())
                self.assertEqual(headers["x-cos-meta-sha256"], module.digest(path))
                self.assertIn("Content-MD5", headers)
                self.assertEqual(requests[-1]["method"], "PUT")


if __name__ == "__main__":
    unittest.main()
