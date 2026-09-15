// Proves the full spawn+tail+chdman pipeline works against the REAL chdman.exe,
// not just the mock used by Task 6's smoke test and the .bat's own regression suite.
use std::process::{Command, Stdio};
use std::{fs, thread, time::Duration};

#[test]
fn spawns_bat_with_real_chdman_and_produces_valid_chd() {
    let repo_root = std::env::current_dir().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    let bat = repo_root.join("convertir_a_chd.bat");
    let real_chdman = repo_root.join("chdman.exe");
    assert!(bat.exists(), "expected {:?} to exist", bat);
    assert!(real_chdman.exists(), "expected {:?} to exist", real_chdman);

    let work = std::env::temp_dir().join("chd_ui_real_chdman_smoke_test");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(work.join("game")).unwrap();

    // A real (tiny) raw audio track chdman can actually parse and convert,
    // not the fake placeholder bytes used against the mock.
    let samples = 2352 * 75; // ~1 CD sector-aligned second of silence
    fs::write(work.join("game/Track.bin"), vec![0u8; samples]).unwrap();
    fs::write(
        work.join("game/Track.cue"),
        "FILE \"Track.bin\" BINARY\nTRACK 01 MODE1/2352\nINDEX 01 00:00:00\n",
    ).unwrap();

    let mut child = Command::new(&bat)
        .arg(&work)
        .env("CHDMAN_OVERRIDE", &real_chdman)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn convertir_a_chd.bat with real chdman.exe");

    child.wait().expect("wait on child");
    thread::sleep(Duration::from_millis(300));

    let log = fs::read_to_string(work.join("conversion_log.txt")).unwrap();
    assert!(log.contains("OK    | "), "expected an OK line in log:\n{}", log);
    let chd_path = work.join("game/Track.chd");
    assert!(chd_path.exists(), ".chd file was not created");
    let chd_size = fs::metadata(&chd_path).unwrap().len();
    assert!(chd_size > 0, ".chd file is empty");

    fs::remove_dir_all(&work).unwrap();
}
