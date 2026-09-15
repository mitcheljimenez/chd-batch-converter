// This is a smoke test of the *process-spawning + tailing* pattern used by
// start_conversion, exercised directly against the mock chdman rather than
// through Tauri's IPC layer (which needs a running window to invoke).
use std::process::{Command, Stdio};
use std::{fs, thread, time::Duration};

#[test]
fn spawns_bat_and_tail_sees_all_expected_lines() {
    let repo_root = std::env::current_dir().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    let bat = repo_root.join("convertir_a_chd.bat");
    let mock = repo_root.join("tests/mock_chdman.bat");
    assert!(bat.exists(), "expected {:?} to exist", bat);
    assert!(mock.exists(), "expected {:?} to exist", mock);

    let work = std::env::temp_dir().join("chd_ui_backend_integration_test");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(work.join("game")).unwrap();
    fs::write(
        work.join("game/Track.cue"),
        "FILE \"Track.bin\" BINARY\nTRACK 01 MODE1/2352\nINDEX 01 00:00:00\n",
    )
    .unwrap();
    fs::write(work.join("game/Track.bin"), "fake bin").unwrap();

    let mut child = Command::new(&bat)
        .arg(&work)
        .env("CHDMAN_OVERRIDE", &mock)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn convertir_a_chd.bat");

    child.wait().expect("wait on child");
    thread::sleep(Duration::from_millis(200)); // allow final log flush

    let log = fs::read_to_string(work.join("conversion_log.txt")).unwrap();
    assert!(log.contains("OK    | "), "expected an OK line in log:\n{}", log);
    assert!(work.join("game/Track.chd").exists());

    fs::remove_dir_all(&work).unwrap();
}
