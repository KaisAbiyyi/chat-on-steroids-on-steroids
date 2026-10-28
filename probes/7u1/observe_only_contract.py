"""Synthetic-only reference model for the 7U1 read-only observation contract.

This is deliberately not a production WebView2/Rust/MV3 adapter. It exercises
fail-closed contract rules without opening a provider, reading an account,
simulating human consent, persisting snapshots, or exposing a Send operation.
"""
from dataclasses import dataclass
import re
from urllib.parse import urlsplit

PROVIDERS = {
    "chatgpt": "https://chatgpt.com",
    "claude": "https://claude.ai",
    "gemini": "https://gemini.google.com",
}
SHA40 = re.compile(r"[0-9a-f]{40}\Z")
SHA64 = re.compile(r"[0-9a-f]{64}\Z")
PROFILE = re.compile(r"7u1-[A-Za-z0-9_-]{8,48}\Z")
OPAQUE_ID = re.compile(r"[A-Za-z0-9_-]{8,128}\Z")
MAX_SNAPSHOT_BYTES = 65536
INVALIDATION_EVENTS = {
    "navigation", "tab-close", "renderer-crash", "host-rebind",
    "worker-retired", "detach",
}


class Refused(Exception):
    """A contract operation was rejected fail-closed."""


@dataclass(frozen=True)
class ObservationContext:
    source_sha: str
    manifest_sha256: str
    provider: str
    origin: str
    profile_id: str
    partition_id: str
    tab_id: int
    document_id: str
    navigation_epoch: int
    document_generation: int
    worker_generation: int
    host_generation: int


@dataclass(frozen=True)
class SyntheticHostAttestation:
    context: ObservationContext
    nonce: str
    test_only: bool = True


@dataclass
class _Lease:
    context: ObservationContext
    last_snapshot_sequence: int = 0


def _exact_origin(origin, provider):
    if not isinstance(origin, str) or provider not in PROVIDERS:
        return False
    try:
        parsed = urlsplit(origin)
        return (
            origin == PROVIDERS[provider]
            and parsed.scheme == "https"
            and parsed.netloc == parsed.hostname
            and parsed.hostname == parsed.hostname.lower()
            and parsed.username is None
            and parsed.password is None
            and parsed.port is None
            and parsed.path == ""
            and parsed.query == ""
            and parsed.fragment == ""
        )
    except ValueError:
        return False


class SyntheticObserveOnlyContractAdapter:
    """In-memory contract harness only; never connects to the production host."""

    def __init__(self, expected_source_sha, expected_manifest_sha256):
        if not SHA40.fullmatch(expected_source_sha or ""):
            raise Refused("invalid expected source SHA")
        if not SHA64.fullmatch(expected_manifest_sha256 or ""):
            raise Refused("invalid expected binary manifest SHA256")
        self._expected_source_sha = expected_source_sha
        self._expected_manifest_sha256 = expected_manifest_sha256
        self._next_attestation = 1
        self._next_lease = 1
        self._used_attestations = set()
        self._leases = {}
        self._revoked = set()
        self._profile_partition = {}
        self._partition_profile = {}
        self._last_generations = {}

    def _validate_context(self, context):
        if not isinstance(context, ObservationContext):
            raise Refused("missing native observation context")
        if context.source_sha != self._expected_source_sha:
            raise Refused("source SHA mismatch")
        if context.manifest_sha256 != self._expected_manifest_sha256:
            raise Refused("binary manifest SHA256 mismatch")
        if not _exact_origin(context.origin, context.provider):
            raise Refused("provider/origin mismatch")
        if not PROFILE.fullmatch(context.profile_id or ""):
            raise Refused("profile is not a dedicated 7U1 profile")
        if not OPAQUE_ID.fullmatch(context.partition_id or ""):
            raise Refused("invalid opaque partition handle")
        if not OPAQUE_ID.fullmatch(context.document_id or ""):
            raise Refused("invalid document identity")
        if isinstance(context.tab_id, bool) or not isinstance(context.tab_id, int) or context.tab_id < 0:
            raise Refused("invalid tab identity")
        generations = (
            context.navigation_epoch, context.document_generation,
            context.worker_generation, context.host_generation,
        )
        if any(isinstance(value, bool) or not isinstance(value, int) or value < 1
               for value in generations):
            raise Refused("invalid navigation/document/worker/host generation")

    def issue_synthetic_attestation(self, context):
        """Issue a one-use test fixture token, not owner consent or production authority."""
        self._validate_context(context)
        nonce = "fixture-attestation-" + str(self._next_attestation)
        self._next_attestation += 1
        return SyntheticHostAttestation(context=context, nonce=nonce)

    def attach(self, context, attestation):
        self._validate_context(context)
        if not isinstance(attestation, SyntheticHostAttestation) or not attestation.test_only:
            raise Refused("production/native attestation required")
        if attestation.context != context:
            raise Refused("attestation context mismatch")
        if attestation.nonce in self._used_attestations:
            raise Refused("attestation replay")
        existing_partition = self._profile_partition.get(context.profile_id)
        if existing_partition is not None and existing_partition != context.partition_id:
            raise Refused("profile partition rebound")
        existing_profile = self._partition_profile.get(context.partition_id)
        if existing_profile is not None and existing_profile != context.profile_id:
            raise Refused("cross-profile partition reuse")
        key = (context.profile_id, context.tab_id)
        prior = self._last_generations.get(key)
        current = (
            context.navigation_epoch, context.document_generation,
            context.worker_generation, context.host_generation,
        )
        if prior is not None and any(now <= old for now, old in zip(current, prior)):
            raise Refused("stale generation or rebind")
        self._used_attestations.add(attestation.nonce)
        self._profile_partition[context.profile_id] = context.partition_id
        self._partition_profile[context.partition_id] = context.profile_id
        self._last_generations[key] = current
        handle = "lease-" + str(self._next_lease).zfill(8)
        self._next_lease += 1
        self._leases[handle] = _Lease(context=context)
        return handle

    def snapshot(self, handle, observed_context, payload, sequence):
        lease = self._leases.get(handle)
        if lease is None or handle in self._revoked:
            raise Refused("stale or revoked observation handle")
        self._validate_context(observed_context)
        if observed_context != lease.context:
            raise Refused("stale document/worker/host generation")
        if isinstance(sequence, bool) or not isinstance(sequence, int) or sequence < 1:
            raise Refused("invalid snapshot sequence")
        if sequence <= lease.last_snapshot_sequence:
            raise Refused("snapshot replay")
        if not isinstance(payload, bytes):
            raise Refused("snapshot must be bytes from the bounded test fixture")
        if len(payload) > MAX_SNAPSHOT_BYTES:
            raise Refused("snapshot exceeds 65536-byte bound")
        lease.last_snapshot_sequence = sequence
        return bytes(payload)

    def revoke(self, handle, event):
        if event not in INVALIDATION_EVENTS:
            raise Refused("unknown lifecycle event")
        if handle not in self._leases or handle in self._revoked:
            raise Refused("stale or replayed revoke")
        del self._leases[handle]
        self._revoked.add(handle)
        return {"revoked": True, "reason": event}

    def detach(self, handle):
        return self.revoke(handle, "detach")

    # Intentionally no send(), click(), submit(), cookie, credential, or
    # arbitrary-script capability exists on this test adapter.
