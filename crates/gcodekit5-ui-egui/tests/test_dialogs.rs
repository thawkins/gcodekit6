//! The dialog queue never blocks: a request waits across frames until craft-dialogs answers it,
//! and each answer is delivered once, tagged with what it was asked for.

use std::path::PathBuf;
use std::sync::Arc;

use craft_dialogs::{Answer, Kind, MessageResult, Request};
use gcodekit5_app::i18n::Translator;
use gcodekit5_ui_egui::dialogs::{
    ConfirmChoice, ConfirmRequest, DialogAnswer, Dialogs, FileFilter, FilePurpose,
};

fn dialogs() -> Dialogs {
    Dialogs::new(Translator::new("en"), Arc::new(|| {}))
}

/// The end of a frame without a window, as the test hook answers before any native dialog.
fn end_frame(dialogs: &mut Dialogs) {
    dialogs.end_frame(None::<&eframe::Frame>);
}

#[test]
fn test_request_survives_frames() {
    let mut dialogs = dialogs();
    let path = PathBuf::from("/work/part.gcode");

    // Frame 1: the request is queued, nothing is answered.
    dialogs.open_file(
        FilePurpose::OpenGcode,
        &[FileFilter::GCODE, FileFilter::ALL],
    );
    assert_eq!(dialogs.poll(), None);
    assert_eq!(dialogs.waiting(), 1);

    // Frame 2: still waiting.
    assert_eq!(dialogs.poll(), None);
    assert_eq!(dialogs.waiting(), 1);

    // Frame 3: craft-dialogs' override answers the request in place of a native dialog.
    let answer = path.clone();
    dialogs.set_override(Some(Box::new(move |request: &Request| {
        match request {
            Request::File(file) => {
                assert_eq!(file.kind, Kind::Open { multiple: false });
                assert_eq!(file.title.as_deref(), Some("Open G-Code File"));
                let names: Vec<&str> = file.filters.iter().map(|f| f.name.as_str()).collect();
                assert_eq!(names, ["G-Code Files", "All Files"]);
            }
            Request::Message(message) => panic!("unexpected message box {message:?}"),
        }
        Answer::Paths(vec![answer.clone()])
    })));
    end_frame(&mut dialogs);
    assert_eq!(
        dialogs.poll(),
        Some(DialogAnswer::File {
            purpose: FilePurpose::OpenGcode,
            path,
        })
    );
    assert_eq!(dialogs.waiting(), 0);

    // Frame 4: delivered once.
    end_frame(&mut dialogs);
    assert_eq!(dialogs.poll(), None);
}

#[test]
fn test_cancel_and_confirm_answers() {
    let mut dialogs = dialogs();
    dialogs.set_override(Some(Box::new(|request: &Request| match request {
        Request::File(_) => Answer::Cancelled,
        Request::Message(_) => Answer::Message(MessageResult::No),
    })));
    dialogs.save_file(FilePurpose::SaveDesign, &[FileFilter::DESIGN]);
    dialogs.confirm(ConfirmRequest::UnsavedDesign);
    end_frame(&mut dialogs);
    end_frame(&mut dialogs);
    assert_eq!(
        dialogs.poll(),
        Some(DialogAnswer::Cancelled {
            purpose: FilePurpose::SaveDesign
        })
    );
    assert_eq!(
        dialogs.poll(),
        Some(DialogAnswer::Confirm {
            request: ConfirmRequest::UnsavedDesign,
            choice: ConfirmChoice::No,
        })
    );
    assert_eq!(dialogs.poll(), None);
}
