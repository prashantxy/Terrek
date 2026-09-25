//! Runs real shells through the PTY with Terrek's hooks and checks what gets recorded.
//! Skips shells that aren't installed.
#![cfg(unix)]

use std::io::Read;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use terrek::db::CommandRecord;
use terrek::pty::{ShellProcess, SpawnOptions};
use terrek::sessions::recorders::{FedPiece, Recorder};

fn which(program: &str) -> Option<String> {
    ["/bin", "/usr/bin", "/usr/local/bin", "/opt/homebrew/bin"]
        .iter()
        .map(|dir| format!("{dir}/{program}"))
        .find(|path| std::path::Path::new(path).exists())
}

/// Type `commands` into `program` and collect the records until `expected` arrive.
fn run_shell(program: &str, commands: &[&str], expected: usize) -> (Vec<CommandRecord>, String) {
    let home = tempfile::tempdir().unwrap();
    // A clean HOME keeps the user's own rc files out of the test.
    std::env::set_var("HOME", home.path());
    std::env::remove_var("ZDOTDIR");
    std::env::remove_var(terrek::SESSION_ENV);
    std::env::set_current_dir(home.path()).unwrap();

    let mut shell = ShellProcess::spawn(SpawnOptions {
        program,
        integration: true,
        session_id: "test",
        cols: 120,
        rows: 40,
    })
    .unwrap();

    let mut reader = shell.reader().unwrap();
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    let mut recorder = Recorder::new("test".into(), true, 4096);
    let mut records = Vec::new();
    let mut screen = Vec::new();
    let mut pending = commands.iter();
    let deadline = Instant::now() + Duration::from_secs(20);

    while records.len() < expected && Instant::now() < deadline {
        if recorder.at_prompt {
            if let Some(cmd) = pending.next() {
                recorder.user_submitted();
                shell.write(format!("{cmd}\r").as_bytes()).unwrap();
            }
        }
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) {
            for piece in recorder.feed(&chunk).pieces {
                match piece {
                    FedPiece::Output(bytes) => screen.extend(bytes),
                    FedPiece::Finished(record) => records.push(record),
                }
            }
        }
    }
    let _ = shell.write(b"exit\r");
    (records, String::from_utf8_lossy(&screen).into_owned())
}

fn check(program: &str) {
    let Some(path) = which(program) else {
        eprintln!("skipping: {program} not installed");
        return;
    };
    let (records, screen) = run_shell(
        &path,
        &[
            "echo hello-terrek",
            "ls /definitely/not/here",
            "",
            "cd /tmp && false",
        ],
        3,
    );
    assert_eq!(
        records.len(),
        3,
        "{program}: records {records:#?}\nscreen:\n{screen}"
    );

    assert_eq!(records[0].command, "echo hello-terrek");
    assert_eq!(records[0].exit_code, Some(0));
    assert_eq!(
        records[0].output.as_deref(),
        Some("hello-terrek"),
        "{program}"
    );

    assert_eq!(records[1].command, "ls /definitely/not/here");
    assert_ne!(records[1].exit_code, Some(0));
    let output = records[1].output.as_deref().unwrap_or("");
    assert!(
        output.contains("No such file"),
        "{program}: output was {output:?}"
    );

    // The empty line in between produced no record.
    assert_eq!(records[2].command, "cd /tmp && false");
    assert_eq!(records[2].exit_code, Some(1));
    assert!(
        records[2].cwd.as_deref().unwrap_or("").ends_with("tmp"),
        "{:?}",
        records[2].cwd
    );

    assert!(
        !screen.contains("7777"),
        "{program}: markers leaked to the screen:\n{screen}"
    );
}

// One test so the process-wide HOME/cwd changes never race.
#[test]
fn zsh_and_bash_hooks_record_commands() {
    check("zsh");
    check("bash");
}
