//! Explicit disposable-server qualification. Never selects a personal SSH profile.
use cos_exec::{
    process::{Launch, Manager},
    ssh,
};
use cos_tools::remote::{RemoteFiles, Sftp};
use serde_json::json;
use std::{io, path::Path, time::Duration};
fn main() -> io::Result<()> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 5 {
        return Err(io::Error::other(
            "Expected good-config bad-host-config bad-auth-config remote-root local-cwd",
        ));
    }
    let remote = json!({"host":"cos-rust-public-ssh","path":a[3]});
    let open = |config: &str| {
        Sftp::spawn(
            &ssh::program(),
            &ssh::args(
                &remote,
                &["-F".into(), config.into(), "-s".into()],
                &["sftp".into()],
            ),
            Some(Path::new(&a[4])),
        )
    };
    let mut r = RemoteFiles {
        sftp: open(&a[0])?,
        root: a[3].clone(),
    };
    assert_eq!(r.read("public.txt", 1024)?, b"public SSH fixture\n");
    assert!(r.read("escape.txt", 1024).is_err());
    r.apply_patch("*** Begin Patch\n*** Update File: public.txt\n@@\n-public SSH fixture\n+public SSH edited\n*** Add File: created.txt\n+new public file\n*** End Patch",None)?;
    assert_eq!(r.read("created.txt", 1024)?, b"new public file\n");
    drop(r);
    let bad_host = match open(&a[1]) {
        Ok(_) => panic!("Changed host key accepted"),
        Err(e) => e,
    };
    assert!(bad_host.to_string().contains("not trusted"), "{bad_host}");
    let bad_auth = match open(&a[2]) {
        Ok(_) => panic!("Unauthorized key accepted"),
        Err(e) => e,
    };
    assert!(
        bad_auth.to_string().contains("did not accept your SSH key"),
        "{bad_auth}"
    );
    let manager = Manager::default();
    let args = ssh::args(
        &remote,
        &["-F".into(), a[0].clone(), "-T".into()],
        &[ssh::command(
            &a[3],
            "printf 'public remote command\\n'; printf '%s\\n' \"$PWD\"; exit 7",
        )],
    );
    let mut result = manager.launch(
        Launch {
            owner: "public-ssh-fixture",
            program: &ssh::program(),
            args: &args,
            cwd: Path::new(&a[4]),
            tty: false,
            raw_cmd: false,
        },
        Duration::from_secs(3),
    )?;
    let mut output = result["output"].as_str().unwrap_or_default().to_owned();
    while let Some(id) = result["session_id"].as_i64() {
        result = manager.poll(id as i32, "public-ssh-fixture", "", Duration::from_secs(5))?;
        output.push_str(result["output"].as_str().unwrap_or_default());
    }
    assert_eq!(result["exit_code"], 7, "{result}");
    assert!(output.contains("public remote command"), "{output}");
    assert!(output.contains(&a[3]), "{output}");
    manager.shutdown();
    println!(
        "{}",
        json!({"status":"PASS","checks":["real public-key SSH authentication and Host alias","known-host key change refused","unauthorized identity refused","SFTP read and patch over SSH","remote symlink escape refused","remote command cwd/output/nonzero exit"],"server":"disposable loopback sshd via private Docker stdio proxy","personalProfilesUsed":false})
    );
    Ok(())
}
