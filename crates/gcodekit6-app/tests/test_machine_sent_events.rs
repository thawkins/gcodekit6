//! `MachineEvent::Sent`: manual lines and labelled real-time bytes are reported when written;
//! program lines, status polls and overrides are not.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gcodekit6_app::machine::{MachineController, MachineEvent, RealtimeCommand, realtime_label};
use gcodekit6_app::test_support::{LoopbackShared, loopback_controller, tcp_settings};

/// Polls until `want` events of the form `Sent` were seen (or 10 s passed); returns them.
fn sent_events(controller: &mut MachineController, want: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut sent = Vec::new();
    while Instant::now() < deadline && sent.len() < want {
        for event in controller.poll() {
            if let MachineEvent::Sent(text) = event {
                sent.push(text);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    sent
}

fn connected() -> MachineController {
    let shared = Arc::new(LoopbackShared::default());
    let mut controller = loopback_controller(&shared, Duration::from_millis(1));
    controller.connect(&tcp_settings()).expect("connect");
    controller
}

#[test]
fn test_sent_reports_manual_lines_in_order() {
    let mut controller = connected();
    controller.send_line("$H").expect("send $H");
    controller.send_line("G0 X1").expect("send mdi");
    controller.jog(gcodekit6_app::machine::Axis::X, 1.0, 1000.0);
    assert_eq!(
        sent_events(&mut controller, 3),
        vec!["$H", "G0 X1", "$J=G91 G21 X1.00 F1000"]
    );
}

#[test]
fn test_sent_reports_labelled_realtime_only() {
    let mut controller = connected();
    controller.realtime(RealtimeCommand::StatusQuery);
    controller.realtime(RealtimeCommand::FeedOverridePlus10);
    controller.realtime(RealtimeCommand::FeedHold);
    controller.realtime(RealtimeCommand::CycleStart);
    controller.realtime(RealtimeCommand::JogCancel);
    controller.stop();
    assert_eq!(
        sent_events(&mut controller, 5),
        vec![
            "! (Feed hold)",
            "~ (Cycle start)",
            "0x85 (Jog cancel)",
            "0x18 (Reset)"
        ]
    );
}

#[test]
fn test_sent_does_not_report_program_lines() {
    let mut controller = connected();
    let program: Arc<[String]> = (0..5).map(|i| format!("G1 X{i}")).collect();
    controller.start_stream(program).expect("start");
    controller.send_line("$X").ok();
    let sent = sent_events(&mut controller, 1);
    assert!(sent.iter().all(|s| !s.starts_with("G1")), "{sent:?}");
}

#[test]
fn test_realtime_label_names() {
    assert_eq!(realtime_label(0x18), Some("0x18 (Reset)"));
    assert_eq!(realtime_label(b'?'), None);
    assert_eq!(realtime_label(0x91), None);
}
