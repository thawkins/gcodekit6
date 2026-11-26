//! Work the Laser Image Engraver does off the UI thread, as GTK did: the engraving itself
//! ([`EngraveJob`], with progress and Cancel) and the decoding of the chosen image for the
//! page's preview ([`PreviewLoad`]).
//!
//! Each runs on its own thread and reports over a channel that the pane polls once per
//! frame, so a large image never stalls a frame. Cancel cannot interrupt the engraver (it
//! has no stop point, as in GTK); the worker finishes and its program is dropped.
//!
//! Dependencies: `gcodekit5-camtools` (engraver), `image` (decoding), `egui` (preview
//! texture).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use gcodekit5_camtools::laser_engraver::EngravingParameters;

/// The longest side of the preview image, in pixels.
const PREVIEW_SIDE: u32 = 512;

/// The msgids the progress row shows, for the translation check.
pub fn msgids() -> Vec<&'static str> {
    vec![
        n_!("Loading image..."),
        n_!("Generating G-code..."),
        n_!("Finalizing..."),
        n_!("Cancel"),
        super::super::bitmap_pane::CANCELLED_BUSY,
    ]
}

/// Engraves `image` with `params` on the calling thread, reporting progress (0 to 1).
///
/// # Arguments
/// * `image` - The image file.
/// * `params` - The engraving parameters.
/// * `progress` - Called with the fraction done.
///
/// # Errors
/// The engraver's refusal (an image it cannot read, parameters it refuses).
pub fn engrave(
    image: &Path,
    params: EngravingParameters,
    progress: impl FnMut(f32),
) -> Result<String, String> {
    super::run_engraver(image, params, progress)
}

/// What an engraving worker reports.
#[derive(Debug)]
enum EngraveMessage {
    /// Fraction done.
    Progress(f32),
    /// The program, or the engraver's refusal.
    Done(Result<String, String>),
}

/// An engraving running on a worker thread.
#[derive(Debug)]
pub struct EngraveJob {
    messages: Receiver<EngraveMessage>,
    cancel: Arc<AtomicBool>,
    /// Set by the worker thread when it has finished.
    finished: Arc<AtomicBool>,
    progress: f32,
}

/// How a poll of an [`EngraveJob`] found it.
#[derive(Debug, PartialEq, Eq)]
pub enum JobState {
    /// Still engraving.
    Running,
    /// Finished with the program, or the engraver's refusal.
    Done(Result<String, String>),
}

impl EngraveJob {
    /// Starts engraving `image` with `params` on a new thread.
    ///
    /// # Arguments
    /// * `image` - The image file.
    /// * `params` - The engraving parameters.
    ///
    /// # Errors
    /// The thread could not be started.
    pub fn start(image: PathBuf, params: EngravingParameters) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        std::thread::Builder::new()
            .name("bitmap-engraving".to_string())
            .spawn(move || {
                let result = engrave(&image, params, |p| {
                    if !stop.load(Ordering::SeqCst) {
                        // The pane may be gone; then nobody needs the progress.
                        let _ = tx.send(EngraveMessage::Progress(p));
                    }
                });
                let _ = tx.send(EngraveMessage::Done(result));
                done.store(true, Ordering::SeqCst);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            messages: rx,
            cancel,
            finished,
            progress: 0.0,
        })
    }

    /// Whether the worker thread has finished (also after a cancel).
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    /// The fraction done so far.
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Tells the worker its program is not wanted; the pane drops the job.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Takes the worker's reports; never blocks.
    pub fn poll(&mut self) -> JobState {
        loop {
            match self.messages.try_recv() {
                Ok(EngraveMessage::Progress(p)) => self.progress = p,
                Ok(EngraveMessage::Done(result)) => return JobState::Done(result),
                Err(TryRecvError::Empty) => return JobState::Running,
                Err(TryRecvError::Disconnected) => {
                    return JobState::Done(Err(super::WORKER_STOPPED.to_string()));
                }
            }
        }
    }

    /// The msgid of what the worker is doing, from its progress (GTK's stages).
    pub fn stage_msgid(&self) -> &'static str {
        if self.progress < 0.1 {
            n_!("Loading image...")
        } else if self.progress < 0.9 {
            n_!("Generating G-code...")
        } else {
            n_!("Finalizing...")
        }
    }
}

/// A decoded preview: the image's size and a reduced copy to show.
struct Decoded {
    size: [usize; 2],
    image: egui::ColorImage,
}

/// The chosen image, decoded for the preview on a worker thread.
pub struct PreviewLoad {
    /// The image file.
    path: PathBuf,
    /// The worker's answer, until it arrives.
    pending: Option<Receiver<Result<Decoded, String>>>,
    /// The image's size in pixels, once decoded.
    size: Option<[usize; 2]>,
    /// The preview texture, once decoded.
    texture: Option<egui::TextureHandle>,
    /// The decoded image waiting to become a texture.
    decoded: Option<egui::ColorImage>,
}

impl std::fmt::Debug for PreviewLoad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewLoad")
            .field("path", &self.path)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl PreviewLoad {
    /// Starts decoding `path` on a new thread; an image that cannot be read shows no
    /// preview (as in GTK).
    ///
    /// # Arguments
    /// * `path` - The image file.
    pub fn start(path: &Path) -> Self {
        let (tx, rx) = mpsc::channel();
        let file = path.to_path_buf();
        let started = std::thread::Builder::new()
            .name("bitmap-preview".to_string())
            .spawn(move || {
                let _ = tx.send(decode(&file));
            });
        if let Err(error) = &started {
            tracing::warn!(%error, "bitmap preview not loaded");
        }
        Self {
            path: path.to_path_buf(),
            pending: started.ok().map(|_| rx),
            size: None,
            texture: None,
            decoded: None,
        }
    }

    /// The image file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the worker has not answered yet.
    pub fn loading(&self) -> bool {
        self.pending.is_some()
    }

    /// The image's size in pixels, once decoded.
    pub fn size(&self) -> Option<[usize; 2]> {
        self.size
    }

    /// Takes the worker's answer; never blocks.
    pub fn poll(&mut self) {
        let Some(rx) = self.pending.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(decoded)) => {
                self.size = Some(decoded.size);
                self.decoded = Some(decoded.image);
                self.pending = None;
            }
            Ok(Err(error)) => {
                tracing::warn!(path = %self.path.display(), %error, "bitmap preview not loaded");
                self.pending = None;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.pending = None,
        }
    }

    /// The preview texture, made from the decoded image the first time it is asked for.
    ///
    /// # Arguments
    /// * `ctx` - The egui context that owns the texture.
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<&egui::TextureHandle> {
        if let Some(image) = self.decoded.take() {
            self.texture =
                Some(ctx.load_texture("cam-bitmap-preview", image, egui::TextureOptions::LINEAR));
        }
        self.texture.as_ref()
    }
}

/// Decodes `path` and reduces it to at most [`PREVIEW_SIDE`] pixels a side.
fn decode(path: &Path) -> Result<Decoded, String> {
    let image = image::open(path).map_err(|e| e.to_string())?;
    let size = [image.width() as usize, image.height() as usize];
    let small = image.thumbnail(PREVIEW_SIDE, PREVIEW_SIDE).to_rgba8();
    let shown = [small.width() as usize, small.height() as usize];
    Ok(Decoded {
        size,
        image: egui::ColorImage::from_rgba_unmultiplied(shown, small.as_raw()),
    })
}
