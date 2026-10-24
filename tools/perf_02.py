#!/usr/bin/env python3
"""PERF-02: cloud-only, synthetic browser-mode authority/lifecycle probe.

This module never launches a browser, attaches to a user profile, touches cookies,
sends messages, or measures actual RAM. Inputs are synthetic host attestations:
a production Windows host must independently establish their truth.
"""
from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from typing import Any

MODES = ("external-reuse", "embedded-lazy")
EVENTS = ("open", "idle", "close", "crash", "host_restart")
ALLOWED_FIELDS = {
    "open": frozenset(("kind", "generation", "mode", "explicit_consent",
                       "native_attested", "profile_isolated", "external_lease_verified")),
    "idle": frozenset(("kind", "generation")),
    "close": frozenset(("kind", "generation")),
    "crash": frozenset(("kind", "generation")),
    "host_restart": frozenset(("kind", "generation")),
}
MAX_EVENTS = 4096


class ContractError(ValueError):
    """An untrusted/ambiguous browser-mode transition must not be executed."""


def require(ok: bool, why: str) -> None:
    if not ok:
        raise ContractError(why)


def validate_event(event: Any) -> None:
    require(isinstance(event, dict), "event must be an object")
    kind = event.get("kind")
    require(type(kind) is str and kind in EVENTS, "unknown event")
    require(not (set(event) - ALLOWED_FIELDS[kind]),
            "unexpected input fields (privacy boundary)")
    require(type(event.get("generation")) is int and event["generation"] >= 0,
            "generation must be a nonnegative integer")
    if kind == "open":
        require(event.get("mode") in MODES, "unsupported browser mode")
        for flag in ("explicit_consent", "native_attested", "profile_isolated"):
            require(type(event.get(flag)) is bool, "missing or invalid " + flag)
        if "external_lease_verified" in event:
            require(type(event["external_lease_verified"]) is bool,
                    "invalid external lease flag")
        require(event["explicit_consent"] and event["native_attested"]
                and event["profile_isolated"], "missing consent/authority/isolation")
        if event["mode"] == "external-reuse":
            require(event.get("external_lease_verified") is True,
                    "no native-owned exclusive external session lease")
        else:
            require(event.get("external_lease_verified", False) is False,
                    "embedded mode must not claim an external lease")


@dataclass
class BrowserModeProbe:
    """A host-only transition proposal, not a browser adapter or permission grant."""

    generation: int = 0
    mode: str = "closed"
    launches: int = 0
    external_leases: int = 0
    releases: int = 0

    def transition(self, event: dict[str, Any]) -> list[str]:
        validate_event(event)
        kind, gen = event["kind"], event["generation"]
        if kind == "idle":
            require(gen == self.generation, "stale idle event")
            return []
        if kind == "open":
            require(self.mode == "closed", "browser already active: never auto-switch")
            require(gen > self.generation, "stale/replayed browser generation")
            # A verified lease is a *proposed* native input, never inferred
            # from a running Chrome/Edge/Brave PID, default profile or cookies.
            self.generation = gen
            self.mode = event["mode"]
            if self.mode == "external-reuse":
                self.external_leases += 1
                return ["request_native_external_lease_attach"]
            self.launches += 1
            return ["request_native_isolated_webview2_create"]
        if kind == "host_restart":
            require(gen > self.generation, "stale/replayed host restart")
            effects = self._release()
            self.generation = gen
            return effects
        require(gen == self.generation, "stale lifecycle event")
        if self.mode == "closed":
            # Double close/crash MUST NOT issue duplicate destruction actions.
            return []
        return self._release()

    def _release(self) -> list[str]:
        if self.mode == "external-reuse":
            effect = "request_native_lease_detach_only"
        elif self.mode == "embedded-lazy":
            effect = "request_native_owned_webview2_dispose"
        else:
            return []
        self.mode = "closed"
        self.releases += 1
        return [effect]


def probe(events: Any) -> dict[str, Any]:
    require(type(events) is list and len(events) <= MAX_EVENTS, "invalid event batch")
    session = BrowserModeProbe()
    emitted: list[dict[str, Any]] = []
    for index, event in enumerate(events):
        # Fails the entire probe closed on an invalid transition. No effects
        # from this model execute against the OS, even for earlier events.
        effects = session.transition(event)
        if effects:
            emitted.append({"step": index, "effects": effects})
    return {
        "schema_version": 1,
        "classification": "synthetic-architecture-only",
        "effects": emitted,
        "state": {
            "mode": session.mode,
            "generation": session.generation,
            "embedded_create_requests": session.launches,
            "external_lease_requests": session.external_leases,
            "release_requests": session.releases,
        },
        "performance_winner": "UNDETERMINED",
        "native_product_acceptance": "NOT_RUN",
        "provider_account_acceptance": "NOT_RUN",
        "measurement_protocol": {
            "workloads": ["external_browser", "embedded_browser"],
            "root_policy": "explicit-native-owned-root-plus-attributable-external-roots",
            "external_browser_memory_is_free": False,
            "must_compare": ["same_machine", "same_build", "same_profile_class",
                             "same_workload", "same_duration", "same_power_mode",
                             "full_process_tree", "cold_and_warm", "idle_and_active"],
            "unresolved": ["real_browser_control_feasibility",
                           "real_account_authority_and_consent",
                           "whole_tree_RAM_and_CPU", "physical_lifecycle_cleanup"],
        },
    }


SYNTHETIC_SCENARIO = [
    {"kind": "idle", "generation": 0},
    {"kind": "open", "generation": 1, "mode": "external-reuse",
     "explicit_consent": True, "native_attested": True,
     "profile_isolated": True, "external_lease_verified": True},
    {"kind": "close", "generation": 1},
    {"kind": "open", "generation": 2, "mode": "embedded-lazy",
     "explicit_consent": True, "native_attested": True,
     "profile_isolated": True},
    {"kind": "crash", "generation": 2},
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("smoke", "probe"))
    parser.add_argument("--input", help="synthetic JSON array of events (probe only)")
    args = parser.parse_args()
    if args.command == "probe":
        require(bool(args.input), "--input required")
        with open(args.input, encoding="utf-8") as handle:
            events = json.load(handle)
    else:
        require(not args.input, "smoke takes no input")
        events = SYNTHETIC_SCENARIO
    print(json.dumps(probe(events), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ContractError, ValueError, OSError) as exc:
        parser_error = type(exc).__name__
        # Never print raw input, file content, user paths, or exception values.
        print(json.dumps({"error": parser_error, "native_product_acceptance": "NOT_RUN"}))
        raise SystemExit(2)
