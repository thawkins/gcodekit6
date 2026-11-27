//! The dialog queue never blocks: a request waits across frames until craft-dialogs answers it,
//! and each answer is delivered once, tagged with what it was asked for.

use std::path::PathBuf;
use std::sync::Arc;

use craft_dialogs::{Answer, Kind, MessageResult, Request};
use gcodekit6_app::i18n::Translator;
use gcodekit6_ui_egui::dialogs::{
    Asked, ConfirmChoice, ConfirmRequest, DialogAnswer, Dialogs, FileFilter, FilePurpose,
};

fn dialogs() -> Dialogs {
    Dialogs::new(Translator::new("en"), Arc::new(|| {}))
}

/// The end of a frame without a window, as the test hook answers before any native dialog.
fn end_frame(dialogs: &mut Dialogs) {
    dialogs.end_frame(None::<&eframe::Frame>);
}

/// A queued request waits, unanswered, across frames that do not show it, is answered when
/// the end of a frame shows it, and its answer is delivered once. craft-dialogs' only public
/// hook (`set_override`) answers as soon as the request is shown, so this does not cover a
/// native dialog staying on screen across frames; that is craft-dialogs' own guarantee.
#[test]
fn test_request_waits_and_answer_is_delivered_once() {
    let mut dialogs = dialogs();
    let path = PathBuf::from("/work/part.gcode");

    // Frame 1: the request is queued, nothing is answered.
    dialogs.open_file(FilePurpose::OpenGcode, &FilePurpose::OpenGcode.filters());
    assert_eq!(dialogs.poll(), None);
    assert_eq!(dialogs.waiting(), 1);

    // Frame 2: still waiting.
    assert_eq!(dialogs.poll(), None);
    assert_eq!(dialogs.waiting(), 1);

    // Frame 3: the end of the frame shows it; craft-dialogs' override answers in place of a
    // native dialog.
    let answer = path.clone();
    dialogs.set_override(Some(Box::new(move |request: &Request| {
        match request {
            Request::File(file) => {
                assert_eq!(file.kind, Kind::Open { multiple: false });
                assert_eq!(file.title.as_deref(), Some("Open G-Code File"));
                let names: Vec<&str> = file.filters.iter().map(|f| f.name.as_str()).collect();
                let expected: Vec<&str> = FilePurpose::OpenGcode
                    .filters()
                    .iter()
                    .map(|f| f.name_msgid)
                    .collect();
                assert_eq!(names, expected);
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

/// Open dialogs offer G-Code (or design) files first and an All Files choice that matches
/// every file on this platform's backends; Save dialogs offer only their own type.
#[test]
fn test_filters_per_purpose_on_this_platform() {
    assert_eq!(FilePurpose::SaveGcode.filters(), [FileFilter::GCODE]);
    assert_eq!(FilePurpose::SaveDesign.filters(), [FileFilter::DESIGN]);
    let open_gcode = FilePurpose::OpenGcode.filters();
    let open_design = FilePurpose::OpenDesign.filters();
    match FileFilter::all_files() {
        Some(all) => {
            assert_eq!(open_gcode, [FileFilter::GCODE, all]);
            assert_eq!(open_design, [FileFilter::DESIGN, all]);
            assert_eq!(all.name_msgid, "All Files");
        }
        // No filter form matches every file here: the Open dialogs are unfiltered.
        None => {
            assert!(open_gcode.is_empty());
            assert!(open_design.is_empty());
        }
    }
}

/// On Linux and the BSDs rfd shows the desktop portal, or zenity when there is none. The
/// portal turns the extension `*` into the glob `*`; zenity writes `*.{ext}` for every
/// extension and splits the joined list on spaces, so `" *"` gives it a bare `*`. Both must
/// end up with a glob that also matches names without a dot.
#[cfg(all(unix, not(target_vendor = "apple")))]
#[test]
fn test_all_files_matches_extensionless_on_portal_and_zenity() {
    let all = FileFilter::all_files().expect("an All Files filter on this platform");
    // rfd 0.17.2 xdg_desktop_portal.rs: `*` (or empty) stays `*`, else `*.{ext}`.
    let portal: Vec<String> = all
        .extensions
        .iter()
        .map(|ext| match *ext {
            "*" | "" => "*".to_string(),
            ext => format!("*.{ext}"),
        })
        .collect();
    assert!(portal.iter().any(|glob| glob == "*"), "portal: {portal:?}");
    // rfd 0.17.2 linux/zenity.rs: `NAME | *.{ext} *.{ext} ...`, split on spaces by zenity.
    let zenity = all
        .extensions
        .iter()
        .map(|ext| format!("*.{ext}"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        zenity.split_whitespace().any(|glob| glob == "*"),
        "zenity: {zenity:?}"
    );
}

/// Quit prompts come back tagged as the quit's, apart from the same question asked by any
/// other feature; an `Unavailable` answer says which request it was for.
#[test]
fn test_quit_confirm_tagged_and_unavailable_carries_request() {
    let mut dialogs = dialogs();
    dialogs.set_override(Some(Box::new(|_: &Request| {
        Answer::Message(MessageResult::No)
    })));
    dialogs.confirm_quit(ConfirmRequest::UnsavedGcode);
    dialogs.confirm(ConfirmRequest::UnsavedGcode);
    end_frame(&mut dialogs);
    end_frame(&mut dialogs);
    assert_eq!(
        dialogs.poll(),
        Some(DialogAnswer::QuitConfirm {
            request: ConfirmRequest::UnsavedGcode,
            choice: ConfirmChoice::No,
        })
    );
    assert_eq!(
        dialogs.poll(),
        Some(DialogAnswer::Confirm {
            request: ConfirmRequest::UnsavedGcode,
            choice: ConfirmChoice::No,
        })
    );

    dialogs.set_override(Some(Box::new(|_: &Request| {
        Answer::Unavailable("stopped".into())
    })));
    dialogs.confirm_quit(ConfirmRequest::StopJob);
    dialogs.save_file(FilePurpose::SaveGcode, &[FileFilter::GCODE]);
    end_frame(&mut dialogs);
    end_frame(&mut dialogs);
    assert_eq!(
        dialogs.poll(),
        Some(DialogAnswer::Unavailable {
            asked: Asked::Quit(ConfirmRequest::StopJob),
            reason: "stopped".into(),
        })
    );
    assert_eq!(
        dialogs.poll(),
        Some(DialogAnswer::Unavailable {
            asked: Asked::File(FilePurpose::SaveGcode),
            reason: "stopped".into(),
        })
    );
}
