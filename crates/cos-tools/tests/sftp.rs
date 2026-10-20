use cos_tools::remote::Sftp;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};
fn u32(v: u32, out: &mut Vec<u8>) {
    out.extend(v.to_be_bytes());
}
fn string(v: &[u8], out: &mut Vec<u8>) {
    u32(v.len() as u32, out);
    out.extend(v);
}
fn send(s: &mut TcpStream, p: &[u8]) {
    s.write_all(&(p.len() as u32).to_be_bytes()).unwrap();
    s.write_all(p).unwrap();
}
#[test]
fn public_sftp_v3_protocol_fixture() {
    fixture(false, false);
}
#[test]
fn remote_read_checks_the_open_handle_type() {
    fixture(true, false);
}
#[test]
fn non_utf8_sftp_realpath_is_rejected_without_lossy_aliasing() {
    fixture(false, true);
}
fn fixture(directory_handle: bool, bad_path: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    let (mut server, _) = listener.accept().unwrap();
    std::thread::spawn(move || {
        loop {
            let mut len = [0; 4];
            if server.read_exact(&mut len).is_err() {
                break;
            }
            let n = u32::from_be_bytes(len) as usize;
            assert!(n < 4 * 1024 * 1024);
            let mut p = vec![0; n];
            server.read_exact(&mut p).unwrap();
            let ty = p[0];
            if ty == 1 {
                send(&mut server, &[2, 0, 0, 0, 3]);
                continue;
            }
            let id = &p[1..5];
            let mut r = vec![];
            match ty {
                16 => {
                    r.push(104);
                    r.extend(id);
                    u32(1, &mut r);
                    if bad_path {
                        string(b"/srv/\xff", &mut r);
                        string(b"/srv/\xff", &mut r);
                    } else {
                        string(b"/srv/project", &mut r);
                        string(b"/srv/project", &mut r);
                    }
                    u32(0, &mut r);
                }
                17 | 8 => {
                    r.push(105);
                    r.extend(id);
                    u32(5, &mut r);
                    r.extend(14u64.to_be_bytes());
                    u32(
                        if ty == 8 && directory_handle {
                            0o040755
                        } else {
                            0o100644
                        },
                        &mut r,
                    );
                }
                3 => {
                    r.push(102);
                    r.extend(id);
                    string(b"h", &mut r);
                }
                5 => {
                    let offset = u64::from_be_bytes(p[10..18].try_into().unwrap());
                    if offset == 0 {
                        r.push(103);
                        r.extend(id);
                        string(b"public fixture", &mut r);
                    } else {
                        r.push(101);
                        r.extend(id);
                        u32(1, &mut r);
                        string(b"EOF", &mut r);
                        string(b"", &mut r);
                    }
                }
                _ => {
                    r.push(101);
                    r.extend(id);
                    u32(0, &mut r);
                    string(b"", &mut r);
                    string(b"", &mut r);
                }
            }
            send(&mut server, &r);
        }
    });
    let mut s = Sftp::from_io(client.try_clone().unwrap(), client).unwrap();
    if bad_path {
        let error = s.realpath("~/project").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("not valid UTF-8"));
        return;
    }
    assert_eq!(s.realpath("~/project").unwrap(), "/srv/project");
    assert_eq!(s.stat("/srv/project/a").unwrap()["isFile"], true);
    if directory_handle {
        assert!(s.read("/srv/project/a", 1024).is_err());
    } else {
        assert_eq!(s.read("/srv/project/a", 1024).unwrap(), b"public fixture");
    }
    s.write_new("/srv/project/b", b"new public fixture")
        .unwrap();
}
