//! Shared helpers for tests that drive the app against the loopback machine transport of
//! `gcodekit5_app::test_support`.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use gcodekit5_app::test_support::{LoopbackShared, loopback_controller, tcp_settings};
use gcodekit5_settings::SettingsManager;
use gcodekit5_settings::config::Config;
use gcodekit5_ui_egui::machine::{Machine, MachineStatus};
use gcodekit5_ui_egui::{AppState, GcodeKitApp};

/// Upper bound for "eventually" waits; generous so a loaded machine is not flaky.
pub const EVENTUALLY: Duration = Duration::from_secs(10);

/// Settings that connect the Machine Control pane to the loopback host.
pub fn loopback_config() -> Config {
    Config {
        connection: tcp_settings(),
        ..Config::default()
    }
}

/// App state over a fresh loopback controller that acks each line after `ok_delay`.
pub fn loopback_state(ok_delay: Duration) -> (AppState, Arc<LoopbackShared>) {
    loopback_state_with(loopback_config(), ok_delay)
}

/// [`loopback_state`] with the settings `config`.
pub fn loopback_state_with(config: Config, ok_delay: Duration) -> (AppState, Arc<LoopbackShared>) {
    let shared = Arc::new(LoopbackShared::default());
    let state = AppState::new(
        SettingsManager::detached(config),
        crate::test_smoke::native_decision(),
    )
    .with_machine(loopback_controller(&shared, ok_delay));
    (state, shared)
}

/// Polls `machine` until `cond` holds on its status or [`EVENTUALLY`] passes.
pub fn wait_machine(machine: &mut Machine, cond: impl Fn(&MachineStatus) -> bool) -> bool {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        machine.poll();
        if cond(machine.status()) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

/// Waits until `cond` holds or [`EVENTUALLY`] passes.
pub fn wait_until(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    false
}

/// Connects `state`'s machine and waits for the connection.
pub fn connect(state: &mut AppState) {
    gcodekit5_ui_egui::machine::lock(state.machine.controller())
        .connect(&tcp_settings())
        .expect("connect");
    assert!(
        wait_machine(&mut state.machine, |s| s.connected),
        "no Connected event"
    );
}

/// Connects `state`'s machine and starts streaming a `lines`-line program with every ack
/// held, so the job keeps running until it is stopped.
pub fn start_held_stream(state: &mut AppState, shared: &LoopbackShared, lines: usize) {
    shared.hold_acks.store(true, Ordering::SeqCst);
    connect(state);
    let program: String = (0..lines).map(|i| format!("G1 X{i} F1000\n")).collect();
    state.machine.set_program(&program).expect("program");
    let program = state.machine.program().cloned().expect("program loaded");
    gcodekit5_ui_egui::machine::lock(state.machine.controller())
        .start_stream(program)
        .expect("start_stream");
    assert!(
        wait_machine(&mut state.machine, |s| s.streaming),
        "stream never started"
    );
}

/// The app in a harness that runs a frame the way eframe does (`logic`, then `ui`), with
/// the theme installed as `GcodeKitApp::new` installs it.
pub fn app_harness(app: GcodeKitApp) -> Harness<'static, GcodeKitApp> {
    let harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 1000.0))
        .build_ui_state(
            |ui, app: &mut GcodeKitApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            app,
        );
    gcodekit5_ui_egui::theme::install(&harness.ctx);
    harness
}

/// Steps `harness` (one frame, then a short sleep) until `cond` holds or [`EVENTUALLY`]
/// passes.
pub fn step_until(
    harness: &mut Harness<'_, GcodeKitApp>,
    cond: impl Fn(&GcodeKitApp) -> bool,
) -> bool {
    let deadline = Instant::now() + EVENTUALLY;
    while Instant::now() < deadline {
        harness.step();
        if cond(harness.state()) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}
