use cos_core::ui_renderer_ipc::{
    MAX_RENDERER_IPC_FRAME_BYTES, RendererIpcAuthenticator, RendererIpcError,
};

const READY: &str =
    r#"{"kind":"ready","rendererNonce":"00112233445566778899aabbccddeeff"}"#;

fn auth(epoch: u64, key_byte: u8) -> RendererIpcAuthenticator {
    RendererIpcAuthenticator::new(epoch, [key_byte; 32]).unwrap()
}

#[test]
fn cross_language_hmac_vector_is_stable() {
    let mut sender = auth(7, 0x0b);
    let frame = sender.sign_outbound(READY).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(
        decoded["mac"].as_str(),
        Some("276dfc24a1e23b4c973d99dc1f9b76df690c5081bb4a66e3b09417d691df07d1")
    );
    let mut receiver = auth(7, 0x0b);
    let verified = receiver.verify_inbound(frame.as_bytes()).unwrap();
    assert_eq!(verified.sequence, 1);
    assert_eq!(verified.payload["kind"], "ready");
}

#[test]
fn malformed_and_oversized_frames_fail_closed() {
    let mut receiver = auth(7, 0x11);
    assert_eq!(
        receiver.verify_inbound(b"{"),
        Err(RendererIpcError::Malformed)
    );
    assert_eq!(
        receiver.verify_inbound(&vec![b'x'; MAX_RENDERER_IPC_FRAME_BYTES + 1]),
        Err(RendererIpcError::Oversize)
    );
    assert_eq!(
        receiver.verify_inbound(
            br#"{"protocol":1,"epoch":7,"sequence":1,"payloadJson":"{}","mac":"00"}"#
        ),
        Err(RendererIpcError::Malformed)
    );
}

#[test]
fn stale_epoch_replay_and_downgrade_are_rejected() {
    let mut sender = auth(7, 0x22);
    let frame = sender.sign_outbound(READY).unwrap();

    let mut stale = auth(8, 0x22);
    assert_eq!(
        stale.verify_inbound(frame.as_bytes()),
        Err(RendererIpcError::Epoch)
    );

    let mut receiver = auth(7, 0x22);
    receiver.verify_inbound(frame.as_bytes()).unwrap();
    assert_eq!(
        receiver.verify_inbound(frame.as_bytes()),
        Err(RendererIpcError::Sequence)
    );

    let mut downgraded: serde_json::Value = serde_json::from_str(&frame).unwrap();
    downgraded["protocol"] = serde_json::json!(0);
    assert_eq!(
        receiver.verify_inbound(serde_json::to_string(&downgraded).unwrap().as_bytes()),
        Err(RendererIpcError::Protocol)
    );
}

#[test]
fn cross_profile_key_isolation_rejects_another_renderer_frame() {
    let mut profile_a = auth(19, 0x31);
    let frame_a = profile_a.sign_outbound(READY).unwrap();
    let mut profile_b = auth(19, 0x32);
    assert_eq!(
        profile_b.verify_inbound(frame_a.as_bytes()),
        Err(RendererIpcError::Authentication)
    );
}

#[test]
fn rebind_retires_old_epoch_and_resets_both_sequences() {
    let mut old_host = auth(41, 0x44);
    let old_frame = old_host.sign_outbound(READY).unwrap();
    let mut host = auth(41, 0x44);
    host.verify_inbound(old_frame.as_bytes()).unwrap();

    host.rebind(42, [0x45; 32]).unwrap();
    assert_eq!(
        host.verify_inbound(old_frame.as_bytes()),
        Err(RendererIpcError::Epoch)
    );
    let new_frame = host.sign_outbound(READY).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&new_frame).unwrap();
    assert_eq!(decoded["sequence"], 1);
    assert_eq!(decoded["epoch"], 42);
}

#[test]
fn outbound_encoder_rejects_malformed_inner_json_without_consuming_sequence() {
    let mut sender = auth(7, 0x51);
    assert_eq!(
        sender.sign_outbound("{"),
        Err(RendererIpcError::Malformed)
    );
    let valid = sender.sign_outbound(READY).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&valid).unwrap();
    assert_eq!(decoded["sequence"], 1);
}
