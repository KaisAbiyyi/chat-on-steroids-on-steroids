"""PERF-02 cloud synthetic browser-mode probes; no real WebView2 or account."""
from __future__ import annotations

import copy
import json
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import perf_00
import perf_02 as perf

ROOT = Path(__file__).resolve().parents[1]


def open_event(mode="external-reuse", generation=1):
    event = {"kind": "open", "mode": mode, "generation": generation,
             "explicit_consent": True, "native_attested": True,
             "profile_isolated": True}
    if mode == "external-reuse":
        event["external_lease_verified"] = True
    return event


class BrowserModeProbeTests(unittest.TestCase):
    def test_zero_chat_idle_does_not_create_or_attach_any_browser(self):
        result = perf.probe([{"kind": "idle", "generation": 0}] * 20)
        self.assertEqual(result["effects"], [])
        self.assertEqual(result["state"]["embedded_create_requests"], 0)
        self.assertEqual(result["state"]["external_lease_requests"], 0)
        self.assertEqual(result["performance_winner"], "UNDETERMINED")
        self.assertEqual(result["native_product_acceptance"], "NOT_RUN")
        self.assertFalse(result["measurement_protocol"]["external_browser_memory_is_free"])

    def test_external_reuse_is_explicit_and_only_detaches_on_close(self):
        result = perf.probe([open_event(), {"kind": "close", "generation": 1},
                             {"kind": "close", "generation": 1}])
        effects = [e for item in result["effects"] for e in item["effects"]]
        self.assertEqual(effects, ["request_native_external_lease_attach",
                                   "request_native_lease_detach_only"])
        self.assertNotIn("kill", json.dumps(result).lower())
        self.assertEqual(result["state"]["external_lease_requests"], 1)
        self.assertEqual(result["state"]["release_requests"], 1)

    def test_embedded_instance_created_only_by_explicit_open_and_disposed(self):
        result = perf.probe([{"kind": "idle", "generation": 0},
                             open_event("embedded-lazy"),
                             {"kind": "crash", "generation": 1}])
        effects = [e for item in result["effects"] for e in item["effects"]]
        self.assertEqual(effects, ["request_native_isolated_webview2_create",
                                   "request_native_owned_webview2_dispose"])
        self.assertEqual(result["state"]["embedded_create_requests"], 1)

    def test_no_fallback_from_external_to_embedded_without_new_consent(self):
        state = perf.BrowserModeProbe()
        invalid = open_event()
        invalid["external_lease_verified"] = False
        with self.assertRaisesRegex(perf.ContractError, "external session lease"):
            state.transition(invalid)
        self.assertEqual(state.mode, "closed")
        self.assertEqual(state.launches, 0)

    def test_missing_consent_native_attestation_and_isolation_all_deny(self):
        for flag in ("explicit_consent", "native_attested", "profile_isolated"):
            for mode in perf.MODES:
                invalid = open_event(mode)
                invalid[flag] = False
                with self.subTest(mode=mode, flag=flag), self.assertRaises(perf.ContractError):
                    perf.probe([invalid])
                invalid[flag] = "yes"
                with self.assertRaises(perf.ContractError):
                    perf.probe([invalid])

    def test_replay_generation_or_parallel_open_denied(self):
        state = perf.BrowserModeProbe()
        state.transition(open_event())
        with self.assertRaisesRegex(perf.ContractError, "already active"):
            state.transition(open_event("embedded-lazy", 2))
        state.transition({"kind": "close", "generation": 1})
        with self.assertRaisesRegex(perf.ContractError, "stale/replayed"):
            state.transition(open_event("embedded-lazy", 1))
        self.assertEqual(state.launches, 0)

    def test_crash_and_host_restart_release_and_refuse_stale_events(self):
        state = perf.BrowserModeProbe()
        state.transition(open_event("embedded-lazy"))
        self.assertEqual(state.transition({"kind": "host_restart", "generation": 2}),
                         ["request_native_owned_webview2_dispose"])
        self.assertEqual(state.mode, "closed")
        with self.assertRaises(perf.ContractError):
            state.transition({"kind": "crash", "generation": 1})
        with self.assertRaises(perf.ContractError):
            state.transition(open_event("embedded-lazy", 2))
        state.transition(open_event("embedded-lazy", 3))
        self.assertEqual(state.launches, 2)

    def test_external_host_restart_never_kills_shared_browser(self):
        result = perf.probe([open_event(), {"kind": "host_restart", "generation": 2}])
        self.assertEqual(result["effects"][-1]["effects"],
                         ["request_native_lease_detach_only"])
        self.assertEqual(result["state"]["mode"], "closed")

    def test_reject_untrusted_metadata_pids_urls_and_credentials(self):
        for bad_key in ("profile_path", "cookie", "access_token",
                        "command_line", "pid", "url", "account", "prompt"):
            invalid = open_event()
            invalid[bad_key] = "private"
            with self.subTest(key=bad_key), self.assertRaisesRegex(
                    perf.ContractError, "privacy boundary"):
                perf.probe([invalid])
        with self.assertRaises(perf.ContractError):
            perf.probe([{"kind": "navigate", "generation": 0}])

    def test_reject_bad_generation_schema_and_excess_events(self):
        for generation in (-1, True, 2.3, "1", None):
            with self.subTest(generation=generation), self.assertRaises(perf.ContractError):
                perf.probe([{"kind": "idle", "generation": generation}])
        with self.assertRaises(perf.ContractError):
            perf.probe([{"kind": "idle", "generation": 0}] * (perf.MAX_EVENTS + 1))
        with self.assertRaises(perf.ContractError):
            perf.probe({"kind": "idle", "generation": 0})

    def test_privileged_permissions_and_send_never_in_mode_contract(self):
        result = perf.probe(perf.SYNTHETIC_SCENARIO)
        effects = [effect for row in result["effects"] for effect in row["effects"]]
        self.assertFalse(any(word in effect for effect in effects
                             for word in ("send", "login", "cookies", "navigate",
                                          "debugger", "permission", "import")))
        self.assertEqual(result["provider_account_acceptance"], "NOT_RUN")
        self.assertEqual(result["state"]["mode"], "closed")

    def test_perf00_external_process_tree_must_not_be_ignored(self):
        trace = json.loads((ROOT / "fixtures/perf-00/synthetic-trace.json")
                           .read_text(encoding="utf-8"))
        perf_00.validate_trace(trace)
        sample = trace["samples"][0]
        totals = perf_00.sample_tree(sample)
        self.assertTrue(totals["complete"])
        self.assertGreater(totals["external_rss_bytes"], 0)
        self.assertEqual(totals["total_rss_bytes"],
                         totals["owned_rss_bytes"] + totals["external_rss_bytes"])
        bad_sample = copy.deepcopy(sample)
        external_root = next(r for r in bad_sample["roots"] if r["role"] == "external")
        external_root["identity"] = "stale-external-pid"
        self.assertFalse(perf_00.sample_tree(bad_sample)["complete"])

    def test_cli_synthetic_smoke_no_native_pass(self):
        completed = subprocess.run(
            [sys.executable, str(ROOT / "tools/perf_02.py"), "smoke"],
            capture_output=True, text=True, check=True)
        report = json.loads(completed.stdout)
        self.assertEqual(report["native_product_acceptance"], "NOT_RUN")
        self.assertEqual(report["performance_winner"], "UNDETERMINED")
        self.assertEqual(report["state"]["embedded_create_requests"], 1)
        self.assertEqual(report["state"]["external_lease_requests"], 1)

    def test_cli_rejects_unsafe_input_without_echoing_it(self):
        # No user files; isolated synthetic stdin fixture-equivalent is not needed.
        response = subprocess.run([sys.executable, str(ROOT / "tools/perf_02.py"),
                                   "probe"], capture_output=True, text=True)
        self.assertEqual(response.returncode, 2)
        self.assertNotIn("Traceback", response.stderr)


if __name__ == "__main__":
    unittest.main()
