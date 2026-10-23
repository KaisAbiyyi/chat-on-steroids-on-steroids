//! 7E source-wire differential qualification. Synthetic fixtures only:
//! Rust 7B receipt admission must agree with 7C JS event parser for the
//! shared corpus. Neither harness is a real browser/owner qualification.
use cos_core::browser_protocol::{DocumentIdentity, MAX_NORMALIZED_FRAME_BYTES};
use cos_core::native_browser_bridge::{BridgeBootstrap, NativeBrowserBridge, TrustedHostEvidence};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Corpus {
    schema: u8,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    id: String,
    event: Value,
    accepted: bool,
}

fn host() -> TrustedHostEvidence {
    TrustedHostEvidence {
        identity: DocumentIdentity {
            profile_id: "profile-A".into(),
            tab_id: 3,
            frame_id: 0,
            document_id: "doc-A".into(),
            navigation_epoch: 1,
            session_id: "local-A".into(),
            session_epoch: 1,
            worker_generation: 1,
        },
        origin: "https://synthetic.example".into(),
        host_generation: 1,
    }
}
fn frame(host: &TrustedHostEvidence) -> Vec<u8> {
    serde_json::to_vec(&json!({"protocol":14,"identity":host.identity})).unwrap()
}
fn authenticated() -> (NativeBrowserBridge, TrustedHostEvidence, BridgeBootstrap) {
    let mut bridge = NativeBrowserBridge::new();
    let host = host();
    let bootstrap = bridge.register_host(host.clone()).unwrap();
    bridge
        .complete_handshake(
            &host,
            bootstrap.channel_id,
            bootstrap.host_generation,
            &bootstrap.handshake_proof(),
        )
        .unwrap();
    let observation = frame(&host);
    bridge
        .observe(
            &host,
            bootstrap.channel_id,
            bootstrap.host_generation,
            1,
            &observation,
            &bootstrap.observation_mac(1, &observation),
        )
        .unwrap();
    (bridge, host, bootstrap)
}
fn payload(host: &TrustedHostEvidence, event: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "frame":{"protocol":14,"identity":host.identity},
        "event": event
    }))
    .unwrap()
}
fn attach_event() -> Value {
    json!({"protocol":14,"kind":"attach","sequence":1})
}

#[test]
fn shared_signed_wire_corpus_matches_js_admission() {
    let cases: Corpus =
        serde_json::from_str(include_str!("../../../fixtures/phase7/7e-wire-corpus.json")).unwrap();
    assert_eq!(cases.schema, 1);
    assert!(cases.cases.len() >= 30, "corpus must be nontrivial");
    for case in cases.cases {
        let (mut bridge, host, bootstrap) = authenticated();
        let bytes = payload(&host, case.event);
        let mac = bootstrap.companion_event_mac(2, &bytes);
        let result =
            bridge.authorize_companion_event(&host, bootstrap.channel_id, 1, 2, &bytes, &mac);
        assert_eq!(
            result.is_ok(),
            case.accepted,
            "native/JS disagreement for {}",
            case.id
        );
        if case.accepted {
            assert_eq!(
                result.unwrap(),
                bootstrap.companion_receipt_mac(2, &bytes),
                "receipt binding for {}",
                case.id
            );
            assert!(
                bridge
                    .authorize_companion_event(&host, bootstrap.channel_id, 1, 2, &bytes, &mac,)
                    .is_err(),
                "replay for {}",
                case.id
            );
        } else {
            // A rejected envelope must not consume bridge sequence or block
            // an authenticated valid retry; no silent success or duplicate Send.
            let good = payload(&host, attach_event());
            let good_mac = bootstrap.companion_event_mac(2, &good);
            assert!(
                bridge
                    .authorize_companion_event(&host, bootstrap.channel_id, 1, 2, &good, &good_mac,)
                    .is_ok(),
                "rejected message consumed sequence for {}",
                case.id
            );
        }
    }
}

#[test]
fn deterministic_transport_mutations_fail_closed_without_consuming_sequence() {
    for seed in 0_u64..224 {
        let (mut bridge, host, bootstrap) = authenticated();
        let good = payload(&host, attach_event());
        let mut mutated = good.clone();
        let mut signed = bootstrap.companion_event_mac(2, &good);
        let sequence = if seed % 7 == 0 { 3 } else { 2 };
        let generation = if seed % 7 == 1 { 2 } else { 1 };
        let mut spoof = host.clone();
        match seed % 7 {
            0 | 1 => {}
            2 => {
                let index = seed as usize % signed.len();
                signed[index] ^= 1;
            }
            3 => {
                let index = seed as usize % mutated.len();
                mutated[index] ^= 1; // MAC mismatch
            }
            4 => spoof.identity.tab_id = 4, // host attestation
            5 => {
                mutated = vec![b' '; MAX_NORMALIZED_FRAME_BYTES + 1];
                signed = bootstrap.companion_event_mac(2, &mutated);
            }
            _ => {
                mutated = payload(&host, json!({"protocol":13,"kind":"attach","sequence":1}));
                signed = bootstrap.companion_event_mac(2, &mutated);
            }
        }
        let result = bridge.authorize_companion_event(
            &spoof,
            bootstrap.channel_id,
            generation,
            sequence,
            &mutated,
            &signed,
        );
        assert!(result.is_err(), "bad transport mutation {seed} accepted");
        let good_mac = bootstrap.companion_event_mac(2, &good);
        assert!(
            bridge
                .authorize_companion_event(&host, bootstrap.channel_id, 1, 2, &good, &good_mac,)
                .is_ok(),
            "mutation {seed} advanced a sequence or retired valid ownership"
        );
    }
}

#[test]
fn reconnect_requires_fresh_host_proof_and_never_resurrects_old_authority() {
    let (mut bridge, old, first) = authenticated();
    let old_bytes = payload(&old, attach_event());
    let old_mac = first.companion_event_mac(2, &old_bytes);
    assert!(
        bridge
            .authorize_companion_event(&old, first.channel_id, 1, 2, &old_bytes, &old_mac,)
            .is_ok()
    );

    let mut next = old.clone();
    next.host_generation = 2;
    next.identity.worker_generation = 2;
    next.identity.navigation_epoch = 2;
    next.identity.document_id = "doc-B".into();
    let replacement = bridge.register_host(next.clone()).unwrap();
    assert!(
        !bridge.retire_host(first.channel_id),
        "old teardown revoked replacement"
    );
    assert!(
        bridge
            .authorize_companion_event(
                &old,
                first.channel_id,
                1,
                3,
                &old_bytes,
                &first.companion_event_mac(3, &old_bytes),
            )
            .is_err()
    );
    let fresh = frame(&next);
    assert!(
        bridge
            .observe(
                &next,
                replacement.channel_id,
                2,
                1,
                &fresh,
                &replacement.observation_mac(1, &fresh),
            )
            .is_err(),
        "reconnect without handshake admitted"
    );
    bridge
        .complete_handshake(
            &next,
            replacement.channel_id,
            2,
            &replacement.handshake_proof(),
        )
        .unwrap();
    bridge
        .observe(
            &next,
            replacement.channel_id,
            2,
            1,
            &fresh,
            &replacement.observation_mac(1, &fresh),
        )
        .unwrap();
    let stale = payload(&old, attach_event());
    assert!(
        bridge
            .authorize_companion_event(
                &next,
                replacement.channel_id,
                2,
                2,
                &stale,
                &replacement.companion_event_mac(2, &stale),
            )
            .is_err(),
        "old document survived replacement"
    );
    let current = payload(&next, attach_event());
    let mac = replacement.companion_event_mac(2, &current);
    assert!(
        bridge
            .authorize_companion_event(&next, replacement.channel_id, 2, 2, &current, &mac,)
            .is_ok()
    );
    assert!(bridge.retire_host(replacement.channel_id));
    assert!(
        bridge
            .authorize_companion_event(
                &next,
                replacement.channel_id,
                2,
                3,
                &current,
                &replacement.companion_event_mac(3, &current),
            )
            .is_err()
    );
}
