use std::process::Command;
use std::time::Duration;
use wait_timeout::ChildExt;

#[test]
fn start_ui_binary_smoke_test() {
    // Build and run the binary; MainWindow::run is a stub that exits quickly.
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args(&["run", "--bin", "gcodekit_ui", "--quiet"]);

    let spawn_res = cmd.spawn();
    if let Err(e) = spawn_res {
        panic!("failed to spawn cargo run: {}", e);
    }

    let mut child = spawn_res.unwrap();
    match child.wait_timeout(Duration::from_secs(5)) {
        Ok(Some(status)) => {
            let code = status.code().unwrap_or(-1);
            assert!(code == 0 || code == -1, "UI binary exit code unexpected: {}", code);
        }
        Ok(None) => {
            // timed out; attempt to kill
            let _ = child.kill();
            panic!("UI binary did not exit within timeout");
        }
        Err(e) => panic!("wait_timeout failed: {}", e),
    }
}
