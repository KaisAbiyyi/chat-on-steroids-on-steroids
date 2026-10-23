//! 7D: native-authorized direct browser debugger/snapshot tool boundary.
//!
//! No page/content-script message grants a lease. The caller must supply the
//! independently attested 7L1 host evidence and a fresh, MAC-protected 7B
//! authorization for EACH tool operation. This does not implement an arbitrary
//! CDP evaluator, provider Send, or user input. Platform integrations implement
//! BrowserDebugger only with a privileged browser-owned transport.
use crate::browser_protocol::{DocumentIdentity, MAX_SAFE_JS_INTEGER};
use crate::native_browser_bridge::{NativeBridgeError, NativeBrowserBridge, TrustedHostEvidence};
use std::collections::HashMap;

pub const MAX_SNAPSHOT_TEXT_BYTES: usize = 256 * 1024;
const MAX_SNAPSHOT_URL_BYTES: usize = 2048;
const MAX_SNAPSHOT_TITLE_BYTES: usize = 2048;
const MAX_DEBUGGER_LEASES: usize = 512;

/// A snapshot MUST be captured from the native debugger session, never supplied
/// by a page message. Its document identity is checked again after acquisition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSnapshot {
    pub identity: DocumentIdentity,
    pub url: String,
    pub title: String,
    pub text: String,
}

/// Callers supply all full 7B signed evidence in one request. Constructing
/// this value does not confer trust; verification is enforced on every use.
pub struct ToolAuthorization<'a> {
    pub trusted: &'a TrustedHostEvidence,
    pub channel_id: u64,
    pub generation: u64,
    pub sequence: u64,
    pub signed_frame: &'a [u8],
    pub mac: [u8; 32],
}

impl ToolAuthorization<'_> {
    fn verify(&self, bridge: &mut NativeBrowserBridge) -> Result<(), NativeBridgeError> {
        bridge.authorize_current(
            self.trusted,
            self.channel_id,
            self.generation,
            self.sequence,
            self.signed_frame,
            &self.mac,
        )
    }
}

/// Only privileged native browser integration should implement this trait.
/// All inputs carry the current native evidence, including exact frame identity.
/// No arbitrary JS execution, clicks, text input, or network requests permitted.
pub trait BrowserDebugger {
    fn attach(&mut self, trusted: &TrustedHostEvidence) -> Result<(), String>;
    fn detach(&mut self, trusted: &TrustedHostEvidence) -> Result<(), String>;
    fn snapshot(&mut self, trusted: &TrustedHostEvidence) -> Result<BrowserSnapshot, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BrowserToolError {
    #[error(transparent)]
    Bridge(#[from] NativeBridgeError),
    #[error("debugger is already owned by another channel, document or frame")]
    Busy,
    #[error("no matching owned debugger lease")]
    Lease,
    #[error("debugger capacity reached")]
    Capacity,
    #[error("invalid debugger snapshot or cross-origin frame")]
    Snapshot,
    #[error("invalid or exhausted debugger handle")]
    Handle,
    #[error("native debugger operation failed: {0}")]
    Runtime(String),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct TabKey {
    profile: String,
    tab: u32,
}

impl TabKey {
    fn new(e: &TrustedHostEvidence) -> Self {
        Self {
            profile: e.identity.profile_id.clone(),
            tab: e.identity.tab_id,
        }
    }
}

#[derive(Debug, Clone)]
struct Lease {
    evidence: TrustedHostEvidence,
    channel_id: u64,
    generation: u64,
    handle: u64,
}

/// Single browser-owned exclusive debugger lease per profile+tab.
/// A handle is never sufficient to authorize an operation: the current 7B
/// MAC/sequence AND exact 7L1 attestation are rechecked on every request.
pub struct DirectBrowserTools<B: BrowserDebugger> {
    runtime: B,
    leases: HashMap<TabKey, Lease>,
    next_handle: u64,
}

impl<B: BrowserDebugger> DirectBrowserTools<B> {
    pub fn new(runtime: B) -> Self {
        Self {
            runtime,
            leases: HashMap::new(),
            next_handle: 0,
        }
    }

    pub fn active_leases(&self) -> usize {
        self.leases.len()
    }

    /// Only host-proven frame/document may attach; a second contender cannot
    /// steal the debugger, even with its own authenticated channel.
    pub fn attach(
        &mut self,
        bridge: &mut NativeBrowserBridge,
        authorization: ToolAuthorization<'_>,
    ) -> Result<u64, BrowserToolError> {
        authorization.verify(bridge)?;
        let trusted = authorization.trusted;
        let channel_id = authorization.channel_id;
        let generation = authorization.generation;
        let key = TabKey::new(trusted);
        if self.leases.contains_key(&key) {
            return Err(BrowserToolError::Busy);
        }
        if self.leases.len() >= MAX_DEBUGGER_LEASES {
            return Err(BrowserToolError::Capacity);
        }
        let next = self
            .next_handle
            .checked_add(1)
            .filter(|x| *x <= MAX_SAFE_JS_INTEGER)
            .ok_or(BrowserToolError::Handle)?;
        // Reserve before invoking native code: an attach error may mean the
        // debugger attached but its success reply was lost. Never expose that
        // uncertain target to a second contender without verified cleanup.
        self.leases.insert(
            key.clone(),
            Lease {
                evidence: trusted.clone(),
                channel_id,
                generation,
                handle: next,
            },
        );
        self.next_handle = next;
        if let Err(error) = self.runtime.attach(trusted) {
            if self.runtime.detach(trusted).is_ok() {
                self.leases.remove(&key);
            }
            return Err(BrowserToolError::Runtime(error));
        }
        Ok(next)
    }

    /// A renderer-supplied handle, guessed tab ID, or old authenticated channel
    /// cannot read from another owner. Snapshot identity and URL are checked
    /// AFTER capture to close navigation races.
    pub fn snapshot(
        &mut self,
        bridge: &mut NativeBrowserBridge,
        authorization: ToolAuthorization<'_>,
        handle: u64,
    ) -> Result<BrowserSnapshot, BrowserToolError> {
        authorization.verify(bridge)?;
        let trusted = authorization.trusted;
        let channel_id = authorization.channel_id;
        let generation = authorization.generation;
        self.require_lease(trusted, channel_id, generation, handle)?;
        let data = self
            .runtime
            .snapshot(trusted)
            .map_err(BrowserToolError::Runtime)?;
        if data.identity != trusted.identity
            || data.url.len() > MAX_SNAPSHOT_URL_BYTES
            || data.title.len() > MAX_SNAPSHOT_TITLE_BYTES
            || data.text.len() > MAX_SNAPSHOT_TEXT_BYTES
            || data.url.chars().any(char::is_control)
            || data.title.chars().any(char::is_control)
            || !url_belongs_to_origin(&data.url, &trusted.origin)
        {
            // Fail closed on an inconsistent native capture. Release the lease
            // so callers must re-attest/re-attach after a navigation race.
            self.retire_from_native(trusted, channel_id, generation);
            return Err(BrowserToolError::Snapshot);
        }
        Ok(data)
    }

    pub fn detach(
        &mut self,
        bridge: &mut NativeBrowserBridge,
        authorization: ToolAuthorization<'_>,
        handle: u64,
    ) -> Result<(), BrowserToolError> {
        authorization.verify(bridge)?;
        let trusted = authorization.trusted;
        let channel_id = authorization.channel_id;
        let generation = authorization.generation;
        self.require_lease(trusted, channel_id, generation, handle)?;
        // On detach failure, retain the lease (never silently grant a competing
        // controller while the native debugger may still be attached).
        self.runtime
            .detach(trusted)
            .map_err(BrowserToolError::Runtime)?;
        self.leases.remove(&TabKey::new(trusted));
        Ok(())
    }

    /// Call only from native-observed browser lifecycle events (navigation,
    /// tab close, debugger detach, worker retirement). Old events cannot
    /// revoke a replacement channel. Failure to detach does NOT allow a new
    /// lease: the record remains until native cleanup succeeds.
    pub fn retire_from_native(
        &mut self,
        trusted: &TrustedHostEvidence,
        channel_id: u64,
        generation: u64,
    ) -> bool {
        let key = TabKey::new(trusted);
        let Some(lease) = self.leases.get(&key) else {
            return false;
        };
        if lease.evidence != *trusted
            || lease.channel_id != channel_id
            || lease.generation != generation
        {
            return false;
        }
        if self.runtime.detach(trusted).is_err() {
            return false;
        }
        self.leases.remove(&key);
        true
    }

    fn require_lease(
        &self,
        trusted: &TrustedHostEvidence,
        channel_id: u64,
        generation: u64,
        handle: u64,
    ) -> Result<(), BrowserToolError> {
        let lease = self
            .leases
            .get(&TabKey::new(trusted))
            .ok_or(BrowserToolError::Lease)?;
        if lease.evidence != *trusted
            || lease.channel_id != channel_id
            || lease.generation != generation
            || lease.handle != handle
        {
            return Err(BrowserToolError::Lease);
        }
        Ok(())
    }
}

fn url_belongs_to_origin(url: &str, origin: &str) -> bool {
    let Some(rest) = url.strip_prefix(origin) else {
        return false;
    };
    rest.is_empty() || rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser_protocol::{NormalizedIdentityFrame, Observation, PROTOCOL_VERSION};
    use crate::native_browser_bridge::BridgeBootstrap;

    #[derive(Default)]
    struct FakeDebugger {
        attaches: u32,
        detaches: u32,
        fail_attach: bool,
        fail_detach: bool,
        return_foreign: bool,
        return_evil_url: bool,
        too_large: bool,
    }

    impl BrowserDebugger for FakeDebugger {
        fn attach(&mut self, _: &TrustedHostEvidence) -> Result<(), String> {
            if self.fail_attach {
                return Err("attach failed".into());
            }
            self.attaches += 1;
            Ok(())
        }
        fn detach(&mut self, _: &TrustedHostEvidence) -> Result<(), String> {
            if self.fail_detach {
                return Err("detach failed".into());
            }
            self.detaches += 1;
            Ok(())
        }
        fn snapshot(&mut self, e: &TrustedHostEvidence) -> Result<BrowserSnapshot, String> {
            let mut id = e.identity.clone();
            if self.return_foreign {
                id.frame_id += 1;
            }
            Ok(BrowserSnapshot {
                identity: id,
                url: if self.return_evil_url {
                    format!("{}.evil.test/x", e.origin)
                } else {
                    format!("{}/a?q=1", e.origin)
                },
                title: "Safe title".into(),
                text: if self.too_large {
                    "x".repeat(MAX_SNAPSHOT_TEXT_BYTES + 1)
                } else {
                    "visible text".into()
                },
            })
        }
    }

    fn host(profile: &str, tab: u32, frame: u32, generation: u64) -> TrustedHostEvidence {
        TrustedHostEvidence {
            identity: DocumentIdentity {
                profile_id: profile.into(),
                tab_id: tab,
                frame_id: frame,
                document_id: "doc-1".into(),
                navigation_epoch: 1,
                session_id: format!("session-{profile}-{tab}-{frame}"),
                session_epoch: 1,
                worker_generation: generation,
            },
            origin: "https://example.test".into(),
            host_generation: generation,
        }
    }

    fn signed(e: &TrustedHostEvidence) -> Vec<u8> {
        serde_json::to_vec(&NormalizedIdentityFrame {
            protocol: PROTOCOL_VERSION,
            identity: e.identity.clone(),
        })
        .unwrap()
    }

    fn auth<'a>(
        trusted: &'a TrustedHostEvidence,
        bootstrap: &BridgeBootstrap,
        sequence: u64,
        frame: &'a [u8],
    ) -> ToolAuthorization<'a> {
        ToolAuthorization {
            trusted,
            channel_id: bootstrap.channel_id,
            generation: trusted.host_generation,
            sequence,
            signed_frame: frame,
            mac: bootstrap.authorization_mac(sequence, frame),
        }
    }

    fn registered(bridge: &mut NativeBrowserBridge, e: &TrustedHostEvidence) -> BridgeBootstrap {
        let b = bridge.register_host(e.clone()).unwrap();
        bridge
            .complete_handshake(e, b.channel_id, e.host_generation, &b.handshake_proof())
            .unwrap();
        let f = signed(e);
        assert!(matches!(
            bridge.observe(
                e,
                b.channel_id,
                e.host_generation,
                1,
                &f,
                &b.observation_mac(1, &f)
            ),
            Ok(Observation::FirstProof | Observation::Advanced)
        ));
        b
    }

    #[test]
    fn exact_owner_can_attach_snapshot_and_release() {
        let (mut bridge, e) = (NativeBrowserBridge::new(), host("a", 1, 0, 1));
        let b = registered(&mut bridge, &e);
        let mut tools = DirectBrowserTools::new(FakeDebugger::default());
        let f = signed(&e);
        let handle = tools.attach(&mut bridge, auth(&e, &b, 2, &f)).unwrap();
        let result = tools
            .snapshot(&mut bridge, auth(&e, &b, 3, &f), handle)
            .unwrap();
        assert_eq!(result.text, "visible text");
        assert_eq!(result.identity, e.identity);
        assert_eq!(tools.active_leases(), 1);
        tools
            .detach(&mut bridge, auth(&e, &b, 4, &f), handle)
            .unwrap();
        assert_eq!(tools.active_leases(), 0);
        assert_eq!(tools.runtime.detaches, 1);
    }

    #[test]
    fn cross_profile_tab_frame_and_guess_handle_are_denied() {
        let mut bridge = NativeBrowserBridge::new();
        let a = host("a", 1, 0, 1);
        let b = host("b", 1, 0, 1);
        let c = host("a", 2, 0, 1);
        let d = host("a", 1, 1, 1);
        let ba = registered(&mut bridge, &a);
        let bb = registered(&mut bridge, &b);
        let bc = registered(&mut bridge, &c);
        let bd = registered(&mut bridge, &d);
        let mut tools = DirectBrowserTools::new(FakeDebugger::default());
        let fa = signed(&a);
        let handle = tools.attach(&mut bridge, auth(&a, &ba, 2, &fa)).unwrap();
        for (e, proof) in [(&b, &bb), (&c, &bc), (&d, &bd)] {
            let f = signed(e);
            assert_eq!(
                tools.snapshot(&mut bridge, auth(e, &proof, 2, &f), handle),
                Err(BrowserToolError::Lease)
            );
        }
        let f = signed(&a);
        assert_eq!(
            tools.snapshot(&mut bridge, auth(&a, &ba, 3, &f), handle + 1),
            Err(BrowserToolError::Lease)
        );
        assert_eq!(tools.active_leases(), 1);
    }

    #[test]
    fn replay_and_unattested_calls_cannot_run_a_tool() {
        let mut bridge = NativeBrowserBridge::new();
        let e = host("a", 1, 0, 1);
        let b = registered(&mut bridge, &e);
        let mut tools = DirectBrowserTools::new(FakeDebugger::default());
        let f = signed(&e);
        let handle = tools.attach(&mut bridge, auth(&e, &b, 2, &f)).unwrap();
        assert!(matches!(
            tools.snapshot(&mut bridge, auth(&e, &b, 2, &f), handle),
            Err(BrowserToolError::Bridge(_))
        ));
        let wrong = host("a", 1, 0, 1);
        let mut fake = wrong;
        fake.origin = "https://attacker.test".into();
        assert!(matches!(
            tools.snapshot(&mut bridge, auth(&fake, &b, 3, &f), handle),
            Err(BrowserToolError::Bridge(_))
        ));
        assert_eq!(tools.runtime.attaches, 1);
    }

    #[test]
    fn malformed_or_cross_origin_capture_revokes_debugger_lease() {
        for mode in 0..3 {
            let mut bridge = NativeBrowserBridge::new();
            let e = host("a", 1, 0, 1);
            let b = registered(&mut bridge, &e);
            let runtime = FakeDebugger {
                return_foreign: mode == 0,
                return_evil_url: mode == 1,
                too_large: mode == 2,
                ..Default::default()
            };
            let mut tools = DirectBrowserTools::new(runtime);
            let f = signed(&e);
            let h = tools.attach(&mut bridge, auth(&e, &b, 2, &f)).unwrap();
            assert_eq!(
                tools.snapshot(&mut bridge, auth(&e, &b, 3, &f), h),
                Err(BrowserToolError::Snapshot)
            );
            assert_eq!(tools.active_leases(), 0);
            assert_eq!(tools.runtime.detaches, 1);
        }
    }

    #[test]
    fn stale_retirement_cannot_detach_replacement() {
        let mut bridge = NativeBrowserBridge::new();
        let e = host("a", 1, 0, 1);
        let b = registered(&mut bridge, &e);
        let mut tools = DirectBrowserTools::new(FakeDebugger::default());
        let f = signed(&e);
        let h = tools.attach(&mut bridge, auth(&e, &b, 2, &f)).unwrap();
        assert!(!tools.retire_from_native(&e, b.channel_id + 1, 1));
        assert!(!tools.retire_from_native(&e, b.channel_id, 2));
        assert_eq!(tools.active_leases(), 1);
        assert!(tools.retire_from_native(&e, b.channel_id, 1));
        assert!(!tools.retire_from_native(&e, b.channel_id, 1));
        assert_eq!(tools.active_leases(), 0);
        let replacement = host("a", 1, 0, 2);
        let br = registered(&mut bridge, &replacement);
        let next = signed(&replacement);
        let h2 = tools
            .attach(&mut bridge, auth(&replacement, &br, 2, &next))
            .unwrap();
        assert_ne!(h, h2);
        assert!(!tools.retire_from_native(&e, b.channel_id, 1));
        assert_eq!(tools.active_leases(), 1);
    }

    #[test]
    fn ambiguous_attach_retains_lease_until_cleanup_succeeds() {
        let mut bridge = NativeBrowserBridge::new();
        let e = host("a", 1, 0, 1);
        let b = registered(&mut bridge, &e);
        let mut tools = DirectBrowserTools::new(FakeDebugger {
            fail_attach: true,
            fail_detach: true,
            ..Default::default()
        });
        let f = signed(&e);
        assert!(matches!(
            tools.attach(&mut bridge, auth(&e, &b, 2, &f)),
            Err(BrowserToolError::Runtime(_))
        ));
        assert_eq!(tools.active_leases(), 1);
        tools.runtime.fail_attach = false;
        assert_eq!(
            tools.attach(&mut bridge, auth(&e, &b, 3, &f)),
            Err(BrowserToolError::Busy)
        );
        tools.runtime.fail_detach = false;
        assert!(tools.retire_from_native(&e, b.channel_id, 1));
        assert_eq!(tools.active_leases(), 0);
        assert!(tools.attach(&mut bridge, auth(&e, &b, 4, &f)).is_ok());
    }

    #[test]
    fn runtime_failures_do_not_enable_concurrent_debuggers() {
        let mut bridge = NativeBrowserBridge::new();
        let e = host("a", 1, 0, 1);
        let b = registered(&mut bridge, &e);
        let mut tools = DirectBrowserTools::new(FakeDebugger {
            fail_attach: true,
            ..Default::default()
        });
        let f = signed(&e);
        assert!(matches!(
            tools.attach(&mut bridge, auth(&e, &b, 2, &f)),
            Err(BrowserToolError::Runtime(_))
        ));
        assert_eq!(tools.active_leases(), 0);
        tools.runtime.fail_attach = false;
        let h = tools.attach(&mut bridge, auth(&e, &b, 3, &f)).unwrap();
        assert_eq!(
            tools.attach(&mut bridge, auth(&e, &b, 4, &f)),
            Err(BrowserToolError::Busy)
        );
        tools.runtime.fail_detach = true;
        assert!(matches!(
            tools.detach(&mut bridge, auth(&e, &b, 5, &f), h),
            Err(BrowserToolError::Runtime(_))
        ));
        assert_eq!(tools.active_leases(), 1);
        assert!(!tools.retire_from_native(&e, b.channel_id, 1));
    }
}
