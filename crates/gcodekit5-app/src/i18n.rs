//! Translations for the front ends, from `po/*.po` without gettext (D2).
//!
//! The implementation lives in `gcodekit5_settings::i18n`, the lowest crate
//! that translates strings (its settings dialog labels), so that both it and
//! this crate share one embedded table without a dependency cycle. Use
//! [`Translator`] directly, or set the process-wide language once with
//! [`set_language`] and translate with [`tr!`](crate::tr) /
//! [`trn!`](crate::trn).

pub use gcodekit5_settings::i18n::{
    Translator, current, needs_restart, normalized_language, set_language,
};
