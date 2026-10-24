//! 15D: bounded, host-owned projections for project/file/agent/extension/settings panels.
//!
//! This module has **no** filesystem, tool, plugin, settings-write or process
//! authority. The privileged host obtains catalog data from existing backend
//! services and must independently authorize every action outside these
//! presentation-only selections. Renderer-supplied identifiers are not paths,
//! capabilities, approvals, or authentication. Native transport identity is
//! attested by the host separately from this protocol.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::ui_contract::{MAX_JS_SAFE_INTEGER, UI_PROTOCOL_VERSION};

pub const MAX_PANEL_IPC_BYTES: usize = 16 * 1024;
pub const MAX_PANEL_CATALOG_BYTES: usize = 64 * 1024;
pub const MAX_PANEL_ITEMS: usize = 256;
const MAX_ID_BYTES: usize = 128;
const MAX_LABEL_BYTES: usize = 256;
const MAX_RELATIVE_PATH_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Panel {
    Projects,
    Files,
    Agents,
    Skills,
    Plugins,
    Settings,
    Tools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingsTab {
    Appearance,
    Language,
    Tools,
    Privacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafePreferences {
    pub theme: Theme,
    pub locale: String,
    pub font_scale_percent: u16,
    pub reduced_motion: bool,
    pub confirm_tool_actions: bool,
}

impl Default for SafePreferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            locale: "en".into(),
            font_scale_percent: 100,
            reduced_motion: false,
            confirm_tool_actions: true,
        }
    }
}

impl SafePreferences {
    fn validate(&self) -> Result<(), PanelError> {
        let locale = self.locale.as_bytes();
        if locale.is_empty()
            || locale.len() > 35
            || !self
                .locale
                .split('-')
                .all(|piece| !piece.is_empty() && piece.len() <= 8 && piece.bytes().all(|b| b.is_ascii_alphanumeric()))
            || !(75..=200).contains(&self.font_scale_percent)
        {
            return Err(PanelError::InvalidCatalog);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectItem {
    pub id: String,
    pub name: String,
    pub remote: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    pub project_id: String,
    /// Display-only relative path. Never pass to a filesystem without a new
    /// backend root/containment/symlink validation at the time of use.
    pub relative_path: String,
    pub kind: FileKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStatus {
    Available,
    Busy,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentItem {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionKind {
    Skill,
    Plugin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionStatus {
    Enabled,
    Disabled,
    ConsentRequired,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionItem {
    pub id: String,
    pub name: String,
    pub kind: ExtensionKind,
    pub status: ExtensionStatus,
}

/// Constructed by privileged host services, not deserialized from renderer.
/// Never include credentials, full paths, OAuth tokens, or arbitrary config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PanelCatalog {
    pub projects: Vec<ProjectItem>,
    pub files: Vec<FileItem>,
    pub agents: Vec<AgentItem>,
    pub extensions: Vec<ExtensionItem>,
    pub preferences: SafePreferences,
}

impl PanelCatalog {
    fn validate(&self) -> Result<(), PanelError> {
        let total = self.projects.len() + self.files.len() + self.agents.len() + self.extensions.len();
        if total > MAX_PANEL_ITEMS {
            return Err(PanelError::InvalidCatalog);
        }
        self.preferences.validate()?;
        let mut project_ids = BTreeSet::new();
        for item in &self.projects {
            valid_id(&item.id)?;
            valid_label(&item.name)?;
            if !project_ids.insert(item.id.as_str()) {
                return Err(PanelError::InvalidCatalog);
            }
        }
        let mut file_keys = BTreeSet::new();
        for item in &self.files {
            valid_id(&item.project_id)?;
            valid_relative_path(&item.relative_path)?;
            if !project_ids.contains(item.project_id.as_str())
                || !file_keys.insert((item.project_id.as_str(), item.relative_path.as_str()))
            {
                return Err(PanelError::InvalidCatalog);
            }
        }
        let mut agent_ids = BTreeSet::new();
        for item in &self.agents {
            valid_id(&item.id)?;
            valid_label(&item.name)?;
            if !agent_ids.insert(item.id.as_str()) {
                return Err(PanelError::InvalidCatalog);
            }
        }
        let mut extension_keys = BTreeSet::new();
        for item in &self.extensions {
            valid_id(&item.id)?;
            valid_label(&item.name)?;
            let kind = match item.kind {
                ExtensionKind::Skill => 0,
                ExtensionKind::Plugin => 1,
            };
            if !extension_keys.insert((kind, item.id.as_str())) {
                return Err(PanelError::InvalidCatalog);
            }
        }
        let len = serde_json::to_vec(self)
            .map_err(|_| PanelError::InvalidCatalog)?
            .len();
        if len > MAX_PANEL_CATALOG_BYTES {
            return Err(PanelError::InvalidCatalog);
        }
        Ok(())
    }
}

fn valid_id(id: &str) -> Result<(), PanelError> {
    if id.is_empty()
        || id.len() > MAX_ID_BYTES
        || id.starts_with('.')
        || id.ends_with('.')
        || id.contains("..")
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
    {
        return Err(PanelError::InvalidIdentifier);
    }
    Ok(())
}

fn valid_label(label: &str) -> Result<(), PanelError> {
    if label.is_empty()
        || label.len() > MAX_LABEL_BYTES
        || label.trim() != label
        || label.chars().any(|c| c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
    {
        return Err(PanelError::InvalidCatalog);
    }
    Ok(())
}

fn valid_relative_path(path: &str) -> Result<(), PanelError> {
    if path.is_empty()
        || path.len() > MAX_RELATIVE_PATH_BYTES
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(|c| c.is_control())
        || path.split('/').any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(PanelError::InvalidIdentifier);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PanelCommand {
    Snapshot {},
    OpenPanel {
        panel: Panel,
        expected_revision: u64,
    },
    SelectProject {
        project_id: String,
        expected_revision: u64,
    },
    SelectFile {
        project_id: String,
        relative_path: String,
        expected_revision: u64,
    },
    SelectAgent {
        agent_id: String,
        expected_revision: u64,
    },
    SelectExtension {
        kind: ExtensionKind,
        extension_id: String,
        expected_revision: u64,
    },
    OpenSettingsTab {
        tab: SettingsTab,
        expected_revision: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelRequest {
    pub protocol: u32,
    pub host_epoch: u64,
    pub sequence: u64,
    pub command: PanelCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelSnapshot {
    pub host_epoch: u64,
    pub revision: u64,
    pub panel: Panel,
    pub settings_tab: SettingsTab,
    pub selected_project: Option<String>,
    pub selected_file: Option<String>,
    pub selected_agent: Option<String>,
    pub selected_extension: Option<(ExtensionKind, String)>,
    pub catalog: PanelCatalog,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelReply {
    pub protocol: u32,
    pub sequence: u64,
    pub state: PanelSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PanelError {
    #[error("panel IPC payload exceeds bound")]
    Oversize,
    #[error("malformed panel IPC payload")]
    Malformed,
    #[error("unsupported panel protocol")]
    Protocol,
    #[error("invalid or stale host epoch")]
    Epoch,
    #[error("out-of-order panel sequence")]
    Sequence,
    #[error("stale panel revision")]
    Revision,
    #[error("unsafe or invalid opaque identifier")]
    InvalidIdentifier,
    #[error("unknown catalog selection")]
    NotFound,
    #[error("invalid host catalog")]
    InvalidCatalog,
    #[error("panel counter exhausted")]
    Exhausted,
}

impl PanelError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Oversize => "payload-too-large",
            Self::Malformed => "invalid-request",
            Self::Protocol => "unsupported-protocol",
            Self::Epoch => "stale-host-epoch",
            Self::Sequence => "invalid-sequence",
            Self::Revision => "stale-revision",
            Self::InvalidIdentifier => "invalid-identifier",
            Self::NotFound => "selection-not-found",
            Self::InvalidCatalog => "invalid-catalog",
            Self::Exhausted => "counter-exhausted",
        }
    }
}

#[derive(Debug)]
pub struct UiPanels {
    snapshot: PanelSnapshot,
    last_sequence: u64,
}

impl UiPanels {
    pub fn new(host_epoch: u64) -> Result<Self, PanelError> {
        check_counter(host_epoch).map_err(|_| PanelError::Epoch)?;
        Ok(Self {
            snapshot: PanelSnapshot {
                host_epoch,
                revision: 1,
                panel: Panel::Projects,
                settings_tab: SettingsTab::Appearance,
                selected_project: None,
                selected_file: None,
                selected_agent: None,
                selected_extension: None,
                catalog: PanelCatalog::default(),
            },
            last_sequence: 0,
        })
    }

    pub fn snapshot(&self) -> PanelSnapshot {
        self.snapshot.clone()
    }

    /// Host-only authoritative catalog replacement; reject invalid snapshots
    /// atomically and drop selections that may refer to stale capabilities.
    pub fn replace_catalog(&mut self, catalog: PanelCatalog) -> Result<(), PanelError> {
        catalog.validate()?;
        let next = next_counter(self.snapshot.revision)?;
        self.snapshot.catalog = catalog;
        self.clear_selections();
        self.snapshot.revision = next;
        Ok(())
    }

    /// Host-only window recreation fence; *erase* catalog to avoid exposing
    /// prior-window project names/metadata in an unauthenticated new renderer.
    pub fn rebind(&mut self, host_epoch: u64) -> Result<(), PanelError> {
        check_counter(host_epoch).map_err(|_| PanelError::Epoch)?;
        if host_epoch <= self.snapshot.host_epoch {
            return Err(PanelError::Epoch);
        }
        let next = next_counter(self.snapshot.revision)?;
        self.snapshot.host_epoch = host_epoch;
        self.snapshot.revision = next;
        self.snapshot.panel = Panel::Projects;
        self.snapshot.settings_tab = SettingsTab::Appearance;
        self.snapshot.catalog = PanelCatalog::default();
        self.clear_selections();
        self.last_sequence = 0;
        Ok(())
    }

    fn clear_selections(&mut self) {
        self.snapshot.selected_project = None;
        self.snapshot.selected_file = None;
        self.snapshot.selected_agent = None;
        self.snapshot.selected_extension = None;
    }

    pub fn dispatch_json(&mut self, bytes: &[u8]) -> Result<PanelReply, PanelError> {
        if bytes.len() > MAX_PANEL_IPC_BYTES {
            return Err(PanelError::Oversize);
        }
        let request: PanelRequest = serde_json::from_slice(bytes).map_err(|_| PanelError::Malformed)?;
        self.dispatch(request)
    }

    pub fn dispatch(&mut self, request: PanelRequest) -> Result<PanelReply, PanelError> {
        if request.protocol != UI_PROTOCOL_VERSION {
            return Err(PanelError::Protocol);
        }
        if request.host_epoch != self.snapshot.host_epoch {
            return Err(PanelError::Epoch);
        }
        check_counter(request.sequence).map_err(|_| PanelError::Sequence)?;
        if request.sequence != next_counter(self.last_sequence).map_err(|_| PanelError::Sequence)? {
            return Err(PanelError::Sequence);
        }
        // A syntactically valid but rejected command consumes its sequence.
        self.last_sequence = request.sequence;
        let mut next = self.snapshot.clone();
        match request.command {
            PanelCommand::Snapshot {} => {}
            PanelCommand::OpenPanel { panel, expected_revision } => {
                expect_revision(&next, expected_revision)?;
                next.panel = panel;
            }
            PanelCommand::SelectProject { project_id, expected_revision } => {
                expect_revision(&next, expected_revision)?;
                valid_id(&project_id)?;
                if !next.catalog.projects.iter().any(|p| p.id == project_id) {
                    return Err(PanelError::NotFound);
                }
                next.panel = Panel::Files;
                next.selected_project = Some(project_id);
                next.selected_file = None;
            }
            PanelCommand::SelectFile { project_id, relative_path, expected_revision } => {
                expect_revision(&next, expected_revision)?;
                valid_id(&project_id)?;
                valid_relative_path(&relative_path)?;
                if !next.catalog.files.iter().any(|f| f.project_id == project_id && f.relative_path == relative_path) {
                    return Err(PanelError::NotFound);
                }
                next.panel = Panel::Files;
                next.selected_project = Some(project_id);
                next.selected_file = Some(relative_path);
            }
            PanelCommand::SelectAgent { agent_id, expected_revision } => {
                expect_revision(&next, expected_revision)?;
                valid_id(&agent_id)?;
                if !next.catalog.agents.iter().any(|a| a.id == agent_id) {
                    return Err(PanelError::NotFound);
                }
                next.panel = Panel::Agents;
                next.selected_agent = Some(agent_id);
            }
            PanelCommand::SelectExtension { kind, extension_id, expected_revision } => {
                expect_revision(&next, expected_revision)?;
                valid_id(&extension_id)?;
                if !next.catalog.extensions.iter().any(|e| e.id == extension_id && e.kind == kind) {
                    return Err(PanelError::NotFound);
                }
                next.panel = match kind { ExtensionKind::Skill => Panel::Skills, ExtensionKind::Plugin => Panel::Plugins };
                next.selected_extension = Some((kind, extension_id));
            }
            PanelCommand::OpenSettingsTab { tab, expected_revision } => {
                expect_revision(&next, expected_revision)?;
                next.panel = Panel::Settings;
                next.settings_tab = tab;
            }
        }
        if next != self.snapshot {
            next.revision = next_counter(self.snapshot.revision)?;
            self.snapshot = next;
        }
        Ok(PanelReply { protocol: UI_PROTOCOL_VERSION, sequence: request.sequence, state: self.snapshot() })
    }
}

fn expect_revision(state: &PanelSnapshot, expected: u64) -> Result<(), PanelError> {
    if state.revision != expected { Err(PanelError::Revision) } else { Ok(()) }
}

fn check_counter(n: u64) -> Result<(), PanelError> {
    if n == 0 || n > MAX_JS_SAFE_INTEGER { Err(PanelError::Exhausted) } else { Ok(()) }
}

fn next_counter(n: u64) -> Result<u64, PanelError> {
    n.checked_add(1).filter(|v| *v <= MAX_JS_SAFE_INTEGER).ok_or(PanelError::Exhausted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> PanelCatalog {
        PanelCatalog {
            projects: vec![ProjectItem { id: "project-1".into(), name: "Demo".into(), remote: false }],
            files: vec![FileItem { project_id: "project-1".into(), relative_path: "src/main.rs".into(), kind: FileKind::File }],
            agents: vec![AgentItem { id: "agent-1".into(), name: "Assistant".into(), status: AgentStatus::Available }],
            extensions: vec![
                ExtensionItem { id: "same-name".into(), name: "Example Skill".into(), kind: ExtensionKind::Skill, status: ExtensionStatus::Enabled },
                ExtensionItem { id: "same-name".into(), name: "Example Plugin".into(), kind: ExtensionKind::Plugin, status: ExtensionStatus::ConsentRequired },
            ],
            ..PanelCatalog::default()
        }
    }

    fn call(ui: &mut UiPanels, sequence: u64, command: PanelCommand) -> Result<PanelReply, PanelError> {
        ui.dispatch(PanelRequest { protocol: UI_PROTOCOL_VERSION, host_epoch: 1, sequence, command })
    }

    #[test]
    fn panels_select_only_catalog_entries() {
        let mut ui = UiPanels::new(1).unwrap();
        ui.replace_catalog(fixture()).unwrap();
        assert_eq!(call(&mut ui, 1, PanelCommand::SelectProject { project_id:"project-1".into(), expected_revision:2 }).unwrap().state.panel, Panel::Files);
        assert_eq!(call(&mut ui, 2, PanelCommand::SelectFile { project_id:"project-1".into(), relative_path:"src/main.rs".into(), expected_revision:3 }).unwrap().state.selected_file.as_deref(), Some("src/main.rs"));
        assert_eq!(call(&mut ui, 3, PanelCommand::SelectAgent { agent_id:"agent-1".into(), expected_revision:4 }).unwrap().state.panel, Panel::Agents);
        assert_eq!(call(&mut ui, 4, PanelCommand::SelectExtension { kind:ExtensionKind::Plugin, extension_id:"same-name".into(), expected_revision:5 }).unwrap().state.panel, Panel::Plugins);
        assert_eq!(call(&mut ui, 5, PanelCommand::SelectExtension { kind:ExtensionKind::Skill, extension_id:"same-name".into(), expected_revision:6 }).unwrap().state.panel, Panel::Skills);
        assert_eq!(call(&mut ui, 6, PanelCommand::OpenSettingsTab { tab: SettingsTab::Privacy, expected_revision:7 }).unwrap().state.settings_tab, SettingsTab::Privacy);
    }

    #[test]
    fn no_op_and_snapshot_do_not_bump_revision() {
        let mut ui = UiPanels::new(1).unwrap();
        let a = call(&mut ui, 1, PanelCommand::Snapshot {}).unwrap();
        let b = call(&mut ui, 2, PanelCommand::OpenPanel { panel: Panel::Projects, expected_revision:1 }).unwrap();
        assert_eq!(a.state.revision, b.state.revision);
    }

    #[test]
    fn invalid_revisions_and_missing_ids_leave_state_unchanged_but_consume_sequence() {
        let mut ui = UiPanels::new(1).unwrap();
        let before = ui.snapshot();
        assert_eq!(call(&mut ui, 1, PanelCommand::OpenPanel { panel: Panel::Plugins, expected_revision: 0 }).unwrap_err(), PanelError::Revision);
        assert_eq!(call(&mut ui, 1, PanelCommand::Snapshot {}).unwrap_err(), PanelError::Sequence);
        assert_eq!(call(&mut ui, 2, PanelCommand::SelectProject { project_id:"unknown".into(), expected_revision:1 }).unwrap_err(), PanelError::NotFound);
        assert_eq!(ui.snapshot(), before);
        assert_eq!(call(&mut ui, 3, PanelCommand::Snapshot {}).unwrap().sequence, 3);
    }

    #[test]
    fn rejects_protocol_epoch_and_replay() {
        let mut ui = UiPanels::new(1).unwrap();
        let mut req = PanelRequest { protocol: 999, host_epoch: 1, sequence:1, command: PanelCommand::Snapshot {} };
        assert_eq!(ui.dispatch(req.clone()).unwrap_err(), PanelError::Protocol);
        req.protocol = UI_PROTOCOL_VERSION;
        req.host_epoch = 2;
        assert_eq!(ui.dispatch(req.clone()).unwrap_err(), PanelError::Epoch);
        req.host_epoch = 1;
        ui.dispatch(req.clone()).unwrap();
        assert_eq!(ui.dispatch(req).unwrap_err(), PanelError::Sequence);
    }

    #[test]
    fn stale_epoch_is_fenced_and_catalog_erased() {
        let mut ui = UiPanels::new(1).unwrap();
        ui.replace_catalog(fixture()).unwrap();
        assert_eq!(ui.rebind(1).unwrap_err(), PanelError::Epoch);
        ui.rebind(2).unwrap();
        assert!(ui.snapshot().catalog.projects.is_empty());
        assert_eq!(call(&mut ui, 1, PanelCommand::Snapshot {}).unwrap_err(), PanelError::Epoch);
        assert_eq!(ui.dispatch(PanelRequest { protocol:UI_PROTOCOL_VERSION, host_epoch:2, sequence:1, command:PanelCommand::Snapshot {} }).unwrap().sequence, 1);
    }

    #[test]
    fn json_rejects_oversize_unknown_fields_unknown_actions_and_duplicate_keys() {
        let mut ui = UiPanels::new(1).unwrap();
        assert_eq!(ui.dispatch_json(&vec![b'x'; MAX_PANEL_IPC_BYTES + 1]).unwrap_err(), PanelError::Oversize);
        assert_eq!(ui.dispatch_json(br#"{"protocol":1,"hostEpoch":1,"sequence":1,"command":{"action":"snapshot"},"extra":1}"#).unwrap_err(), PanelError::Malformed);
        assert_eq!(ui.dispatch_json(br#"{"protocol":1,"hostEpoch":1,"sequence":1,"command":{"action":"execute-tool"}}"#).unwrap_err(), PanelError::Malformed);
        assert_eq!(ui.dispatch_json(br#"{"protocol":1,"hostEpoch":1,"sequence":1,"sequence":2,"command":{"action":"snapshot"}}"#).unwrap_err(), PanelError::Malformed);
        let ok = json!({"protocol":1,"hostEpoch":1,"sequence":1,"command":{"action":"snapshot"}});
        assert_eq!(ui.dispatch_json(&serde_json::to_vec(&ok).unwrap()).unwrap().sequence, 1);
    }

    #[test]
    fn disallow_traversal_drive_paths_and_unknown_file_selections() {
        let mut ui = UiPanels::new(1).unwrap();
        ui.replace_catalog(fixture()).unwrap();
        for path in ["../secret","src/../secret","C:/Windows","/absolute","src\\main.rs","src//main.rs","src/."] {
            assert_eq!(valid_relative_path(path), Err(PanelError::InvalidIdentifier));
        }
        assert_eq!(call(&mut ui, 1, PanelCommand::SelectFile { project_id:"project-1".into(), relative_path:"src/unknown.rs".into(), expected_revision:2 }).unwrap_err(), PanelError::NotFound);
        assert!(ui.snapshot().selected_file.is_none());
    }

    #[test]
    fn catalog_rejects_orphans_duplicates_and_control_characters_atomically() {
        let mut ui = UiPanels::new(1).unwrap();
        ui.replace_catalog(fixture()).unwrap();
        let original = ui.snapshot();
        let mut catalog = fixture();
        catalog.files[0].project_id = "missing".into();
        assert_eq!(ui.replace_catalog(catalog).unwrap_err(), PanelError::InvalidCatalog);
        let mut catalog = fixture();
        catalog.projects.push(catalog.projects[0].clone());
        assert_eq!(ui.replace_catalog(catalog).unwrap_err(), PanelError::InvalidCatalog);
        let mut catalog = fixture();
        catalog.agents[0].name = "Spoof\u{202e}".into();
        assert_eq!(ui.replace_catalog(catalog).unwrap_err(), PanelError::InvalidCatalog);
        assert_eq!(ui.snapshot(), original);
    }

    #[test]
    fn catalog_bounds_and_safe_preferences_enforced() {
        let mut ui = UiPanels::new(1).unwrap();
        let mut catalog = fixture();
        catalog.preferences.font_scale_percent = 201;
        assert_eq!(ui.replace_catalog(catalog).unwrap_err(), PanelError::InvalidCatalog);
        let mut catalog = fixture();
        catalog.preferences.locale = "en--US".into();
        assert_eq!(ui.replace_catalog(catalog).unwrap_err(), PanelError::InvalidCatalog);
        let mut catalog = fixture();
        catalog.agents[0].id = "../bad".into();
        assert_eq!(ui.replace_catalog(catalog).unwrap_err(), PanelError::InvalidIdentifier);
        let mut catalog = fixture();
        for i in 0..=MAX_PANEL_ITEMS {
            catalog.agents.push(AgentItem { id:format!("agent-{i}"), name:"A".into(), status:AgentStatus::Offline });
        }
        assert_eq!(ui.replace_catalog(catalog).unwrap_err(), PanelError::InvalidCatalog);
    }

    #[test]
    fn replacement_clears_stale_selections() {
        let mut ui = UiPanels::new(1).unwrap();
        ui.replace_catalog(fixture()).unwrap();
        call(&mut ui, 1, PanelCommand::SelectProject { project_id:"project-1".into(), expected_revision:2 }).unwrap();
        ui.replace_catalog(PanelCatalog::default()).unwrap();
        assert!(ui.snapshot().selected_project.is_none());
        assert!(ui.snapshot().catalog.files.is_empty());
    }

    #[test]
    fn counters_fence_out_of_range_values() {
        assert_eq!(UiPanels::new(0).unwrap_err(), PanelError::Epoch);
        assert_eq!(UiPanels::new(MAX_JS_SAFE_INTEGER + 1).unwrap_err(), PanelError::Epoch);
        let mut ui = UiPanels::new(1).unwrap();
        assert_eq!(call(&mut ui, MAX_JS_SAFE_INTEGER + 1, PanelCommand::Snapshot {}).unwrap_err(), PanelError::Sequence);
    }
}
