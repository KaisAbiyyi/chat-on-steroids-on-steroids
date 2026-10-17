use cos_core::{config::Root, persistence::DurableStore};
use cos_tools::project::Projects;
use serde_json::json;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn setup() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    DurableStore,
    Projects,
    serde_json::Value,
) {
    let profile = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let durable = DurableStore::open(profile.path()).unwrap();
    let projects = Projects::open(durable.clone());
    let roots = [Root {
        name: "p".into(),
        path: folder.path().to_string_lossy().into(),
        remote: None,
    }];
    let p = projects.add(&roots, "/p").unwrap();
    (profile, folder, durable, projects, p)
}

#[test]
fn color_and_removal_preserve_workspace_and_request_identity() {
    let (_profile, folder, durable, projects, p) = setup();
    let id = p["id"].as_str().unwrap();
    let colored = projects.set_color(id, Some("teal")).unwrap();
    assert_eq!(colored["color"], "teal");
    assert_eq!(colored["path"], p["path"]);
    assert_eq!(colored["chatgpt"], p["chatgpt"]);
    assert!(projects.set_color(id, Some("injected-color")).is_err());
    assert!(projects.set_color(id, None).unwrap().get("color").is_none());
    assert_eq!(projects.remove(id).unwrap()["ungrouped"], true);
    assert!(folder.path().is_dir());
    assert_eq!(projects.pending_creates(now()).unwrap(), json!([]));
    drop(projects);
    let restored = Projects::open(durable);
    assert_eq!(restored.list().unwrap()[0]["chatgpt"], p["chatgpt"]);
    let roots = [Root {
        name: "p".into(),
        path: folder.path().to_string_lossy().into(),
        remote: None,
    }];
    assert_eq!(
        restored.workspace(&roots, id).unwrap().real,
        std::fs::canonicalize(folder.path()).unwrap()
    );
    assert_eq!(restored.add(&roots, "/p").unwrap()["id"], id);
}

#[test]
fn claims_are_exclusive_expire_at_boundary_and_armed_attempts_never_auto_retry() {
    let (_profile, _folder, _durable, projects, p) = setup();
    let at = now();
    let request = p["chatgpt"]["requestId"].as_str().unwrap();
    let changed = projects.subscribe_link_changes().unwrap();
    assert!(
        projects
            .create_transition(request, "claim", "page-a", "", at)
            .unwrap()
    );
    changed.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(
        !projects
            .create_transition(request, "claim", "page-b", "", at + 59_999)
            .unwrap()
    );
    assert!(
        !projects
            .create_transition(request, "arm", "page-a", "", at + 60_000)
            .unwrap()
    );
    assert_eq!(
        projects.pending_creates(at + 60_000).unwrap()[0]["state"],
        "requested"
    );
    assert!(
        projects
            .create_transition(request, "claim", "page-b", "", at + 60_000)
            .unwrap()
    );
    assert!(
        projects
            .create_transition(request, "arm", "page-b", "", at + 60_001)
            .unwrap()
    );
    assert!(
        !projects
            .create_transition(
                request,
                "complete",
                "page-a",
                "g-p-0123456789abcdef0123456789abcdef",
                at + 60_002
            )
            .unwrap()
    );
    assert!(
        !projects
            .create_transition(request, "complete", "page-b", "not-a-project", at + 60_002)
            .unwrap()
    );
    assert_eq!(
        projects.pending_creates(at + 120_000).unwrap()[0]["state"],
        "creating"
    );
    assert_eq!(projects.pending_creates(at + 120_001).unwrap(), json!([]));
    assert_eq!(projects.list().unwrap()[0]["chatgpt"]["state"], "uncertain");
    assert!(
        !projects
            .create_transition(request, "claim", "page-b", "", at + 120_002)
            .unwrap()
    );
    let retried = projects
        .retry_create(p["id"].as_str().unwrap(), at + 120_002)
        .unwrap()
        .unwrap();
    assert_eq!(retried["chatgpt"]["state"], "requested");
    assert_ne!(retried["chatgpt"]["requestId"], request);
    assert!(
        !projects
            .create_transition(
                request,
                "complete",
                "page-b",
                "g-p-0123456789abcdef0123456789abcdef",
                at + 120_003
            )
            .unwrap()
    );
}

#[test]
fn failure_and_gone_links_preserve_user_retry_authority_across_restart() {
    let (_profile, _folder, durable, projects, p) = setup();
    let at = now();
    let id = p["id"].as_str().unwrap();
    let request = p["chatgpt"]["requestId"].as_str().unwrap();
    assert!(
        projects
            .create_transition(request, "claim", "a", "", at)
            .unwrap()
    );
    assert!(
        projects
            .create_transition(request, "fail", "a", "public failure", at + 1)
            .unwrap()
    );
    assert_eq!(projects.list().unwrap()[0]["chatgpt"]["state"], "failed");
    let retried = projects.retry_create(id, at + 2).unwrap().unwrap();
    let request = retried["chatgpt"]["requestId"].as_str().unwrap();
    assert!(
        projects
            .create_transition(request, "claim", "b", "", at + 3)
            .unwrap()
    );
    assert!(
        projects
            .create_transition(request, "arm", "b", "", at + 4)
            .unwrap()
    );
    let native = "g-p-0123456789abcdef0123456789abcdef";
    assert!(
        projects
            .create_transition(request, "complete", "b", native, at + 5)
            .unwrap()
    );
    assert!(
        !projects
            .mark_gone(id, "g-p-ffffffffffffffffffffffffffffffff", at + 6)
            .unwrap()
    );
    drop(projects);
    let projects = Projects::open(durable);
    assert_eq!(projects.list().unwrap()[0]["chatgpt"]["id"], native);
    assert!(projects.retry_create(id, at + 7).unwrap().is_none());
    assert!(projects.mark_gone(id, native, at + 8).unwrap());
    assert_eq!(projects.list().unwrap()[0]["chatgpt"]["state"], "failed");
    assert_eq!(projects.pending_creates(at + 9).unwrap(), json!([]));
}

#[test]
fn concurrent_claimants_cannot_both_acquire_one_request() {
    let (_profile, _folder, _durable, projects, p) = setup();
    let at = now();
    let request = p["chatgpt"]["requestId"].as_str().unwrap().to_owned();
    let projects = Arc::new(projects);
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let threads = ["a", "b"].map(|owner| {
        let p = projects.clone();
        let b = barrier.clone();
        let r = request.clone();
        std::thread::spawn(move || {
            b.wait();
            p.create_transition(&r, "claim", owner, "", at).unwrap()
        })
    });
    barrier.wait();
    assert_eq!(
        threads
            .into_iter()
            .filter_map(|t| t.join().unwrap().then_some(()))
            .count(),
        1
    );
}

#[test]
fn invalid_persisted_link_or_color_is_rejected_instead_of_reoffered() {
    let (_profile, _folder, durable, projects, p) = setup();
    for invalid in [
        json!({"state":"creating","requestId":p["chatgpt"]["requestId"],"updatedAt":0}),
        json!({"state":"linked","id":"javascript:fake","updatedAt":0}),
    ] {
        let mut broken = p.clone();
        broken["chatgpt"] = invalid;
        durable.write_now("projects", json!([broken])).unwrap();
        assert!(projects.list().is_err());
    }
    let mut broken = p;
    broken["color"] = json!("unrecognized");
    durable.write_now("projects", json!([broken])).unwrap();
    assert!(projects.list().is_err());
}

#[test]
fn restored_armed_attempt_expires_without_a_browser_poll() {
    let (_profile, _folder, durable, projects, mut p) = setup();
    p["chatgpt"]["state"] = json!("creating");
    p["chatgpt"]["owner"] = json!("closed-page");
    p["chatgpt"]["updatedAt"] = json!(0);
    durable.write_now("projects", json!([p])).unwrap();
    drop(projects);
    let projects = Projects::open(durable);
    let until = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < until
        && projects.list().unwrap()[0]["chatgpt"]["state"] == "creating"
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(projects.list().unwrap()[0]["chatgpt"]["state"], "uncertain");
}

#[test]
fn expiry_leaves_legacy_projects_without_link_metadata_unchanged() {
    let (_profile, _folder, durable, projects, mut p) = setup();
    let mut legacy = p.clone();
    legacy["id"] = json!("cccccccc-cccc-4ccc-8ccc-cccccccccccc");
    legacy.as_object_mut().unwrap().remove("chatgpt");
    p["chatgpt"]["state"] = json!("creating");
    p["chatgpt"]["owner"] = json!("closed-page");
    p["chatgpt"]["updatedAt"] = json!(0);
    durable
        .write_now("projects", json!([p, legacy.clone()]))
        .unwrap();
    projects.expire_armed(now()).unwrap();
    assert_eq!(projects.list().unwrap()[1], legacy);
}

#[test]
fn actual_upstream_project_link_scenarios_match_each_committed_step() {
    use serde_json::Value;
    fn normalize(v: &mut Value) {
        match v {
            Value::String(s)
                if uuid::Uuid::parse_str(s).is_ok()
                    && s != "11111111-1111-4111-8111-111111111111"
                    && s != "22222222-2222-4222-8222-222222222222" =>
            {
                *s = "<new-uuid>".into()
            }
            Value::Array(a) => a.iter_mut().for_each(normalize),
            Value::Object(o) => o.values_mut().for_each(normalize),
            _ => {}
        }
    }
    let scenarios: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/project-links.json"
    ))
    .unwrap();
    for scenario in scenarios.as_array().unwrap() {
        let d = tempfile::tempdir().unwrap();
        let durable = DurableStore::open(d.path()).unwrap();
        durable
            .write_now("projects", scenario["initial"].clone())
            .unwrap();
        let projects = Projects::open(durable);
        for step in scenario["steps"].as_array().unwrap() {
            let a = step["args"].as_array().unwrap();
            let at = step["at"].as_u64().unwrap();
            let arg = |i: usize| a[i].as_str().unwrap();
            let mut actual = match step["method"].as_str().unwrap() {
                "setProjectColor" => projects.set_color(arg(0), a[1].as_str()).unwrap(),
                "removeProject" => projects.remove(arg(0)).unwrap(),
                "pendingProjectCreates" => projects.pending_creates(at).unwrap(),
                "retryProjectCreate" => json!(projects.retry_create(arg(0), at).unwrap()),
                "markProjectGone" => json!(projects.mark_gone(arg(0), arg(1), at).unwrap()),
                method => json!(
                    projects
                        .create_transition(
                            arg(0),
                            match method {
                                "claimProjectCreate" => "claim",
                                "armProjectCreate" => "arm",
                                "completeProjectCreate" => "complete",
                                "failProjectCreate" => "fail",
                                _ => panic!("{method}"),
                            },
                            arg(1),
                            a.get(2).and_then(Value::as_str).unwrap_or(""),
                            at
                        )
                        .unwrap()
                ),
            };
            normalize(&mut actual);
            assert_eq!(actual, step["expected"], "{step}");
            let mut catalog = projects.list().unwrap();
            normalize(&mut catalog);
            assert_eq!(catalog, step["catalog"], "{step}");
        }
    }
}
