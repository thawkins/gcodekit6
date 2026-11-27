//! The pane's side of the Laser Image Engraver: the engraving it runs on a worker thread
//! (started by Generate, Send to Editor or Preview, polled every frame, cancellable), the
//! image preview it decodes off the UI thread, and the Browse request the app answers with
//! the image dialog.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations).

use std::path::Path;
use std::time::Duration;

use gcodekit5_app::i18n::Translator;

use super::bitmap_engraving::job::{EngraveJob, JobState, PreviewLoad};
use super::{CamDelivery, CamError, CamTool, CamToolsPane, Output, PageInput};

/// Notice: a new engraving waits for a cancelled one's worker.
pub(super) const CANCELLED_BUSY: &str =
    n_!("The cancelled engraving is still finishing; try again in a moment.");

/// How often the pane looks at a running engraving while nothing else repaints.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// A running engraving and what to do with its program.
#[derive(Debug)]
struct Running {
    /// The worker.
    job: EngraveJob,
    /// Where the program goes: the page (`None`), or the editor.
    delivery: Option<CamDelivery>,
    /// The page's input it was started from.
    input: PageInput,
}

/// The Laser Image Engraver's work off the UI thread.
#[derive(Debug, Default)]
pub(super) struct BitmapRun {
    /// The engraving, while it runs.
    running: Option<Running>,
    /// A cancelled engraving whose worker may still be running; a new one waits for it, so
    /// repeated Generate and Cancel never stack CPU-bound workers.
    cancelled: Option<EngraveJob>,
    /// The chosen image's preview.
    preview: Option<PreviewLoad>,
    /// Browse, waiting for the app.
    pub(super) image_request: bool,
}

impl BitmapRun {
    /// The preview texture, once the image is decoded.
    pub(super) fn preview_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        self.preview.as_mut().and_then(|p| p.texture(ctx)).cloned()
    }
}

impl CamToolsPane {
    /// The Laser Image Engraver page's input.
    pub fn bitmap_engraving_mut(&mut self) -> &mut super::BitmapEngravingParams {
        &mut self.bitmap_engraving
    }

    /// Whether an engraving is running.
    pub fn bitmap_job_running(&self) -> bool {
        self.bitmap_run.running.is_some()
    }

    /// Stops waiting for the running engraving; its program is dropped.
    pub fn cancel_bitmap_job(&mut self) {
        if let Some(run) = self.bitmap_run.running.take() {
            run.job.cancel();
            self.bitmap_run.cancelled = Some(run.job);
        }
    }

    /// Whether an engraving worker is still running, including a cancelled one.
    pub fn bitmap_worker_busy(&self) -> bool {
        self.bitmap_job_running()
            || self
                .bitmap_run
                .cancelled
                .as_ref()
                .is_some_and(|job| !job.finished())
    }

    /// The chosen image's size in pixels, once its preview is decoded.
    pub fn bitmap_preview_size(&self) -> Option<[usize; 2]> {
        self.bitmap_run.preview.as_ref().and_then(PreviewLoad::size)
    }

    /// Takes the Browse request; the app answers it with an image dialog.
    pub fn take_image_request(&mut self) -> bool {
        std::mem::take(&mut self.bitmap_run.image_request)
    }

    /// Takes `image` as the image to engrave (Browse's answer) and starts decoding its
    /// preview.
    ///
    /// # Arguments
    /// * `image` - The chosen file.
    pub fn choose_bitmap_image(&mut self, image: &Path) {
        self.bitmap_engraving.set_image(Some(image.to_path_buf()));
        self.refresh_bitmap_preview();
    }

    /// Starts decoding the page's image for the preview, unless it is the one shown.
    pub(super) fn refresh_bitmap_preview(&mut self) {
        let image = self.bitmap_engraving.image().map(Path::to_path_buf);
        let shown = self.bitmap_run.preview.as_ref().map(PreviewLoad::path);
        if image.as_deref() == shown {
            return;
        }
        self.bitmap_run.preview = image.as_deref().map(PreviewLoad::start);
    }

    /// Starts engraving the page's input on a worker thread; the program goes to the page
    /// (`delivery` `None`) or the editor once done. Refused while one runs or the input has
    /// problems.
    pub(super) fn start_bitmap_job(&mut self, delivery: Option<CamDelivery>, tr: &Translator) {
        if self.bitmap_job_running() {
            return;
        }
        if self.bitmap_worker_busy() {
            self.notice = Some(tr.tr(CANCELLED_BUSY).into_owned());
            return;
        }
        self.bitmap_run.cancelled = None;
        let Ok((image, params)) = self.bitmap_engraving.parameters() else {
            self.notice = Some(CamError::Invalid.message(tr));
            return;
        };
        match EngraveJob::start(image, params) {
            Ok(job) => {
                self.notice = None;
                self.bitmap_run.running = Some(Running {
                    job,
                    delivery,
                    input: self.input_snapshot(CamTool::BitmapEngraving),
                });
            }
            Err(error) => self.notice = Some(CamError::Generator(error).message(tr)),
        }
    }

    /// Takes the workers' reports: a finished engraving goes where it was asked to, and a
    /// decoded image becomes the preview. Keeps frames coming while a worker runs.
    pub(super) fn poll_bitmap(&mut self, ctx: &egui::Context, tr: &Translator) {
        if let Some(preview) = self.bitmap_run.preview.as_mut() {
            preview.poll();
            if preview.loading() {
                ctx.request_repaint_after(POLL_INTERVAL);
            }
        }
        let Some(run) = self.bitmap_run.running.as_mut() else {
            return;
        };
        let JobState::Done(result) = run.job.poll() else {
            ctx.request_repaint_after(POLL_INTERVAL);
            return;
        };
        let Some(run) = self.bitmap_run.running.take() else {
            return;
        };
        match result {
            Ok(gcode) => match run.delivery {
                None => {
                    self.output = Some(Output::new(CamTool::BitmapEngraving, run.input, gcode));
                }
                Some(delivery) => self.request_to_editor(gcode, delivery),
            },
            // The worker's own refusals start with msgids.
            Err(error) => {
                let text = super::bitmap_engraving::refusal_text(&error, tr);
                self.notice = Some(CamError::Generator(text).message(tr));
            }
        }
    }

    /// The running engraving's stage, progress and Cancel.
    pub(super) fn show_bitmap_progress(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        let Some(run) = self.bitmap_run.running.as_ref() else {
            return;
        };
        let (stage, progress) = (run.job.stage_msgid(), run.job.progress());
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.label(tr.tr(stage));
            ui.add(
                egui::ProgressBar::new(progress)
                    .show_percentage()
                    .desired_width(200.0),
            );
            cancel = ui.button(tr.tr("Cancel")).clicked();
        });
        if cancel {
            self.cancel_bitmap_job();
        }
    }
}
