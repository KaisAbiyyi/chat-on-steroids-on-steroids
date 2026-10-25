#!/usr/bin/env python3
"""CoSoS 16E: synthetic, disposable release-transaction matrix.

This module is a TEST HARNESS, not an installer. It never contacts an update
server or selects user data. Release 16B's Authenticode verifier is replaced
only with an explicitly supplied fixture callback. Native install, signature,
WebView2, and real-profile rollback remain separate gates.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tempfile
import uuid

from tools import release_16b


class RehearsalRefused(ValueError):
    pass


class InjectedCrash(RuntimeError):
    pass


_SLOT = re.compile(r"slot-[0-9a-f]{32}\Z")
_SELECTOR_LIMIT = 4096


class DisposableMatrix:
    """Bounded temporary folder, verified slots and atomic active selector.

    Never point this at an existing installation or profile. The workspace is
    always exclusively created using TemporaryDirectory and removed on close.
    The tester must supply a fake signer callback; no real signature passes
    are manufactured in production release_16b's CLI.
    """

    def __init__(self, source_sha, trusted_thumbprint, fake_signer):
        if not callable(fake_signer):
            raise RehearsalRefused("synthetic signer callback required")
        self._temp = tempfile.TemporaryDirectory(prefix="cosos-16e-synthetic-")
        self.root = Path(self._temp.name)
        self.slots = self.root / "slots"
        self.slots.mkdir()
        self.selector = self.root / "active.json"
        self.source_sha = source_sha
        self.publisher = trusted_thumbprint
        self.fake_signer = fake_signer

    def close(self):
        self._temp.cleanup()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def _slot(self, name):
        if not isinstance(name, str) or not _SLOT.fullmatch(name):
            raise RehearsalRefused("malformed selector slot")
        path = self.slots / name
        if path.is_symlink() or not path.is_dir():
            raise RehearsalRefused("missing or linked selector slot")
        return path

    def state(self):
        if not self.selector.exists():
            return {"active": None, "previous": None}
        if self.selector.is_symlink() or self.selector.stat().st_size > _SELECTOR_LIMIT:
            raise RehearsalRefused("unsafe selector")
        try:
            value = json.loads(self.selector.read_text("utf-8"))
        except (OSError, UnicodeError, ValueError) as ex:
            raise RehearsalRefused("corrupt selector") from ex
        if not isinstance(value, dict) or set(value) != {"active", "previous"}:
            raise RehearsalRefused("selector schema mismatch")
        if value["active"] is None or value["active"] == value["previous"]:
            raise RehearsalRefused("invalid active/previous selector")
        self._slot(value["active"])
        if value["previous"] is not None:
            self._slot(value["previous"])
        return value

    @staticmethod
    def _manifest_sha(root):
        manifest = Path(root) / "release.json"
        if (manifest.is_symlink() or not manifest.is_file()
                or manifest.stat().st_size > 64 * 1024):
            raise RehearsalRefused("missing, linked or oversize release manifest")
        return hashlib.sha256(manifest.read_bytes()).hexdigest()

    def _inspect(self, directory, current_version):
        try:
            return release_16b.inspect(
                directory, self.source_sha, current_version, self.publisher,
                signature_check=self.fake_signer
            )
        except (release_16b.Refused, OSError) as ex:
            raise RehearsalRefused("release preflight refused: " + str(ex)) from ex

    def current(self):
        state = self.state()
        if state["active"] is None:
            return None
        # Rollback also needs full integrity/signer recheck, independently of
        # version ordering. This does not call the updater to downgrade.
        return self._inspect(self._slot(state["active"]), "0.0.0")

    def _publish(self, active, previous):
        target = self.root / ("selector-" + uuid.uuid4().hex + ".tmp")
        data = json.dumps({"active": active, "previous": previous}, sort_keys=True)
        try:
            with target.open("x", encoding="utf-8") as stream:
                stream.write(data)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(target, self.selector)
        finally:
            target.unlink(missing_ok=True)

    def install_or_upgrade(self, staging_dir, fail_at=None):
        """Accept only newer releases; all failpoints are deterministic.

        Failures before publication leave the old selector intact. A crash
        after atomic publication leaves the new release active and prior
        release available for explicit rollback.
        """
        before = self.state()
        active = self.current()
        current_version = active["version"] if active is not None else "0.0.0"
        expected_manifest_sha = self._manifest_sha(staging_dir)
        receipt = self._inspect(staging_dir, current_version)
        if self._manifest_sha(staging_dir) != expected_manifest_sha:
            raise RehearsalRefused("staging manifest changed during preflight")
        if fail_at == "after_preflight":
            raise InjectedCrash(fail_at)
        name = "slot-" + uuid.uuid4().hex
        new_slot = self.slots / name
        new_slot.mkdir()
        # Copy the exact preflighted payload. Never follow a symlink.
        # Re-inspection detects non-atomic source mutation during copying.
        try:
            manifest = json.loads((Path(staging_dir) / "release.json").read_text("utf-8"))
            paths = ["release.json"] + [x["path"] for x in manifest["files"]]
            for relative in paths:
                source = release_16b.path_is_safe(Path(staging_dir).absolute(), relative)
                dest = new_slot.joinpath(*relative.split("/"))
                dest.parent.mkdir(parents=True, exist_ok=True)
                with source.open("rb") as reader, dest.open("xb") as writer:
                    shutil.copyfileobj(reader, writer, length=1024 * 1024)
            if fail_at == "after_copy":
                raise InjectedCrash(fail_at)
            if (self._manifest_sha(new_slot) != expected_manifest_sha
                    or self._manifest_sha(staging_dir) != expected_manifest_sha):
                raise RehearsalRefused("staging manifest changed during copy")
            check = self._inspect(new_slot, current_version)
            if check["version"] != receipt["version"]:
                raise RehearsalRefused("copied release version changed")
            if fail_at == "before_publish":
                raise InjectedCrash(fail_at)
            self._publish(name, before["active"])
        except Exception:
            # Orphans are isolated inside this disposable test root.
            # Never alter the last successfully published slot/selector.
            if self.state() == before:
                shutil.rmtree(new_slot)
            raise
        if fail_at == "after_publish":
            raise InjectedCrash(fail_at)
        return self.current()

    def rollback(self, fail_at=None):
        before = self.state()
        if before["previous"] is None:
            raise RehearsalRefused("no known previous release")
        # Selection is permitted only after rechecking retained bytes and
        # signer; it does NOT install a downgraded package.
        restored = self._inspect(self._slot(before["previous"]), "0.0.0")
        if fail_at == "before_publish":
            raise InjectedCrash(fail_at)
        self._publish(before["previous"], before["active"])
        if fail_at == "after_publish":
            raise InjectedCrash(fail_at)
        return restored
