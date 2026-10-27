"""Synthetic-only regressions for the 7U1 observe-only contract model."""
from dataclasses import replace
import unittest

from observe_only_contract import (
    MAX_SNAPSHOT_BYTES,
    ObservationContext,
    Refused,
    SyntheticObserveOnlyContractAdapter,
)

SOURCE_SHA = "a" * 40
MANIFEST_SHA = "b" * 64


def context(**overrides):
    values = dict(
        source_sha=SOURCE_SHA,
        manifest_sha256=MANIFEST_SHA,
        provider="chatgpt",
        origin="https://chatgpt.com",
        profile_id="7u1-fixture0001",
        partition_id="partition-fixture0001",
        tab_id=7,
        document_id="document-fixture01",
        navigation_epoch=1,
        document_generation=1,
        worker_generation=1,
        host_generation=1,
    )
    values.update(overrides)
    return ObservationContext(**values)


class ObserveOnlyContractTests(unittest.TestCase):
    def setUp(self):
        self.adapter = SyntheticObserveOnlyContractAdapter(SOURCE_SHA, MANIFEST_SHA)
        self.context = context()

    def attach(self, ctx=None):
        ctx = ctx or self.context
        attestation = self.adapter.issue_synthetic_attestation(ctx)
        return self.adapter.attach(ctx, attestation), attestation

    def test_bounded_snapshot_uses_exact_attested_context(self):
        handle, _ = self.attach()
        payload = b'{"syntheticFixture":true,"kind":"metadata-only"}'
        self.assertEqual(self.adapter.snapshot(handle, self.context, payload, 1), payload)

    def test_source_sha_mismatch_is_denied(self):
        bad = context(source_sha="c" * 40)
        with self.assertRaisesRegex(Refused, "source SHA mismatch"):
            self.adapter.issue_synthetic_attestation(bad)

    def test_binary_manifest_mismatch_is_denied(self):
        bad = context(manifest_sha256="d" * 64)
        with self.assertRaisesRegex(Refused, "manifest SHA256 mismatch"):
            self.adapter.issue_synthetic_attestation(bad)

    def test_provider_origin_mismatch_and_noncanonical_origin_are_denied(self):
        for bad in (
            context(provider="chatgpt", origin="https://claude.ai"),
            context(origin="https://chatgpt.com/"),
            context(origin="http://chatgpt.com"),
            context(origin="https://chatgpt.com:443"),
            context(origin="https://user@chatgpt.com"),
            context(origin="https://chatgpt.com.evil.invalid"),
        ):
            with self.subTest(origin=bad.origin):
                with self.assertRaisesRegex(Refused, "provider/origin mismatch"):
                    self.adapter.issue_synthetic_attestation(bad)

    def test_cross_profile_partition_reuse_is_denied(self):
        handle, _ = self.attach()
        self.assertTrue(handle.startswith("lease-"))
        other = context(profile_id="7u1-fixture0002", tab_id=8)
        token = self.adapter.issue_synthetic_attestation(other)
        with self.assertRaisesRegex(Refused, "cross-profile partition reuse"):
            self.adapter.attach(other, token)

    def test_stale_snapshot_after_document_or_worker_generation_change_is_denied(self):
        handle, _ = self.attach()
        changed = replace(self.context, navigation_epoch=2, document_generation=2,
                          worker_generation=2, host_generation=2,
                          document_id="document-fixture02")
        with self.assertRaisesRegex(Refused, "stale document/worker/host generation"):
            self.adapter.snapshot(handle, changed, b"synthetic", 1)

    def test_navigation_close_crash_rebind_and_worker_retirement_revoke_handles(self):
        for event in ("navigation", "tab-close", "renderer-crash", "host-rebind", "worker-retired"):
            with self.subTest(event=event):
                adapter = SyntheticObserveOnlyContractAdapter(SOURCE_SHA, MANIFEST_SHA)
                ctx = context()
                handle = adapter.attach(ctx, adapter.issue_synthetic_attestation(ctx))
                adapter.revoke(handle, event)
                with self.assertRaisesRegex(Refused, "stale or revoked"):
                    adapter.snapshot(handle, ctx, b"synthetic", 1)

    def test_replayed_attestation_and_snapshot_sequence_are_denied(self):
        token = self.adapter.issue_synthetic_attestation(self.context)
        handle = self.adapter.attach(self.context, token)
        with self.assertRaisesRegex(Refused, "attestation replay"):
            self.adapter.attach(self.context, token)
        self.adapter.snapshot(handle, self.context, b"synthetic", 1)
        with self.assertRaisesRegex(Refused, "snapshot replay"):
            self.adapter.snapshot(handle, self.context, b"synthetic", 1)

    def test_detach_revokes_and_replayed_detach_is_denied(self):
        handle, _ = self.attach()
        self.assertTrue(self.adapter.detach(handle)["revoked"])
        with self.assertRaisesRegex(Refused, "stale or revoked"):
            self.adapter.snapshot(handle, self.context, b"synthetic", 1)
        with self.assertRaisesRegex(Refused, "stale or replayed revoke"):
            self.adapter.detach(handle)

    def test_snapshot_limit_and_invalid_sequence_are_denied(self):
        handle, _ = self.attach()
        with self.assertRaisesRegex(Refused, "65536-byte bound"):
            self.adapter.snapshot(handle, self.context, b"x" * (MAX_SNAPSHOT_BYTES + 1), 1)
        with self.assertRaisesRegex(Refused, "invalid snapshot sequence"):
            self.adapter.snapshot(handle, self.context, b"synthetic", 0)

    def test_rebind_requires_increasing_all_generations(self):
        first, _ = self.attach()
        self.adapter.revoke(first, "navigation")
        stale = replace(self.context, navigation_epoch=2, document_generation=2,
                        worker_generation=1, host_generation=2,
                        document_id="document-fixture02")
        with self.assertRaisesRegex(Refused, "stale generation or rebind"):
            self.adapter.attach(stale, self.adapter.issue_synthetic_attestation(stale))

    def test_test_adapter_has_no_outgoing_send_capability(self):
        self.assertFalse(hasattr(self.adapter, "send"))
        self.assertFalse(hasattr(self.adapter, "click"))
        self.assertFalse(hasattr(self.adapter, "submit"))


if __name__ == "__main__":
    unittest.main()
