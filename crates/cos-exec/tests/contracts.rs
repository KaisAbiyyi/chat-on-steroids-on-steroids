use cos_exec::{buffer::HeadTail, policy, ssh};
use serde_json::Value;
#[test]
fn original_command_policy_vectors() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase4-6/command-policy.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let cmds = case["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            policy::evaluate(&case["policy"], &cmds, case["shell"].as_str().unwrap()),
            case["expected"],
            "{case}"
        );
    }
}
#[test]
fn original_ssh_helpers() {
    let cases: Value =
        serde_json::from_str(include_str!("../../../fixtures/upstream/phase4-6/ssh.json")).unwrap();
    for c in cases.as_array().unwrap() {
        let r = ssh::parse(
            c["address"].as_str().unwrap(),
            c["folder"].as_str().unwrap(),
        );
        if let Some(error) = c["error"].as_str() {
            assert_eq!(r.unwrap_err(), error);
        } else {
            let r = r.unwrap();
            assert_eq!(r, c["remote"]);
            assert_eq!(
                serde_json::to_value(ssh::args(&r, &["-s".into()], &["sftp".into()])).unwrap(),
                c["args"]
            );
            assert_eq!(
                ssh::command(r["path"].as_str().unwrap(), "printf '%s\\n' '😀 a'"),
                c["command"]
            );
            assert_eq!(ssh::login_shell(r["path"].as_str().unwrap()), c["login"]);
        }
    }
}
#[test]
fn bounded_head_tail_drains_unread() {
    let mut b = HeadTail::new(8);
    b.push(b"0123456");
    b.push(b"789abc");
    assert_eq!(b.take(), "0123\n... 5 bytes omitted ...\n9abc");
    assert_eq!(b.take(), "");
    b.push(b"new");
    assert_eq!(b.take(), "new");
}
#[test]
fn upstream_output_truncation() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase4-6/output-truncation.json"
    ))
    .unwrap();
    for c in cases.as_array().unwrap() {
        assert_eq!(
            cos_exec::process::truncate(
                c["text"].as_str().unwrap(),
                c["tokens"].as_u64().unwrap() as usize * 4
            ),
            c["expected"].as_str().unwrap()
        );
    }
}
