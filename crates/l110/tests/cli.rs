//! ทดสอบ binary จริง: dispatch ผ่าน symlink, exit status, SIGPIPE

use std::os::unix::fs::symlink;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const L110: &str = env!("CARGO_BIN_EXE_l110");

fn tempdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("l110-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn help_and_list_work() {
    let out = Command::new(L110).arg("--help").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("Usage: l110 "));

    let out = Command::new(L110).arg("--list").output().unwrap();
    let list = String::from_utf8_lossy(&out.stdout).into_owned();
    for name in ["echo", "true", "false", "help"] {
        assert!(
            list.lines().any(|l| l == name),
            "{name} missing from --list:\n{list}"
        );
    }
}

#[test]
fn every_listed_applet_answers_help_with_exit_zero_or_its_own_status() {
    let list = Command::new(L110).arg("--list").output().unwrap();
    for name in String::from_utf8_lossy(&list.stdout).lines() {
        let out = Command::new(L110).args([name, "--help"]).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            text.starts_with(&format!("Usage: {name} ")),
            "{name}: {text}"
        );
        // false --help exit 1 ตาม GNU; ที่เหลือ 0
        let want = if name == "false" { 1 } else { 0 };
        assert_eq!(out.status.code(), Some(want), "{name} --help");
    }
}

#[test]
fn symlink_name_selects_the_applet() {
    let dir = tempdir("symlink");
    for name in ["echo", "true", "false"] {
        symlink(L110, dir.join(name)).unwrap();
    }
    let out = Command::new(dir.join("echo"))
        .args(["-n", "via", "symlink"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "via symlink");
    assert_eq!(
        Command::new(dir.join("true")).status().unwrap().code(),
        Some(0)
    );
    assert_eq!(
        Command::new(dir.join("false")).status().unwrap().code(),
        Some(1)
    );

    symlink(L110, dir.join("nonesuch")).unwrap();
    let out = Command::new(dir.join("nonesuch")).output().unwrap();
    assert_eq!(out.status.code(), Some(127));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "nonesuch: applet not found\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closed_pipe_kills_with_sigpipe_instead_of_printing_errors() {
    // ส่งข้อมูลเกิน buffer ของ pipe แล้วปิดฝั่งอ่านทันที
    let chunk = "x".repeat(100_000);
    let args: Vec<&str> = std::iter::repeat_n(chunk.as_str(), 15).collect();
    let mut child = Command::new(L110)
        .arg("echo")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.signal(), Some(13), "status: {:?}", out.status);
    assert!(
        out.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn bad_option_to_l110_itself_prints_usage_hint() {
    let out = Command::new(L110).arg("--nope").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "l110: unrecognized option '--nope'\nTry 'l110 --help' for more information.\n"
    );
}
