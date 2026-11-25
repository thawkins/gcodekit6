//! Translations without gettext (D2).
//!
//! `po/*.po` stays the source of truth: `build.rs` parses each catalog with
//! `polib` and embeds it as a sorted table, so there is no `msgfmt` step, no
//! `.mo` file to install and no `gettext-sys`. [`Translator`] looks strings up
//! in one language's table and falls back to the msgid.
//!
//! The module lives here rather than in `gcodekit5-app` because both this
//! crate (the settings dialog labels) and `gcodekit5-app` translate strings,
//! and `gcodekit5-app` depends on this crate; `gcodekit5-app` re-exports it as
//! `gcodekit5_app::i18n` together with the [`tr!`](crate::tr) and
//! [`trn!`](crate::trn) macros.
//!
//! Model code translates through a process-wide current language
//! ([`set_language`], [`current`]) so it need not pass a `Translator` around.

use std::borrow::Cow;
use std::sync::atomic::{AtomicUsize, Ordering};

/// How a catalog picks a plural form for `n` (its `Plural-Forms` header).
///
/// Only the rules the shipped catalogs use exist; `build.rs` rejects others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluralRule {
    /// `plural=(n != 1)`: de, es (gettext default), it, pt, and English.
    NotOne,
    /// `plural=(n > 1)`: fr (0 is singular).
    GreaterThanOne,
}

impl PluralRule {
    /// The plural form index for `n`.
    fn index(self, n: u64) -> usize {
        let plural = match self {
            Self::NotOne => n != 1,
            Self::GreaterThanOne => n > 1,
        };
        usize::from(plural)
    }
}

/// One language's embedded translations.
#[derive(Debug)]
struct Catalog {
    /// Language code, from the file name (`de` for `po/de.po`).
    lang: &'static str,
    /// The catalog's plural rule.
    rule: PluralRule,
    /// `(msgid, msgstr)`, sorted by msgid.
    entries: &'static [(&'static str, &'static str)],
    /// `(msgid, [msgstr[0], msgstr[1], ...])`, sorted by msgid.
    plurals: &'static [(&'static str, &'static [&'static str])],
}

include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

/// Index into [`CATALOGS`] of the current language; `usize::MAX` for none.
static CURRENT: AtomicUsize = AtomicUsize::new(usize::MAX);

/// Translates strings into one language.
///
/// Cheap to copy: it refers to a table embedded in the binary.
#[derive(Debug, Clone, Copy, Default)]
pub struct Translator {
    catalog: Option<&'static Catalog>,
}

impl Translator {
    /// Creates a translator for `lang`.
    ///
    /// # Arguments
    /// * `lang` - a language code such as `"de"`, a locale such as
    ///   `"de_DE.UTF-8"`, or `"system"` / `""` for the language the
    ///   environment asks for (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`).
    ///   A language without a catalog (including English, the source
    ///   language) translates every string to its msgid.
    pub fn new(lang: &str) -> Self {
        Self {
            catalog: catalog_index(lang).and_then(|i| CATALOGS.get(i)),
        }
    }

    /// The language code of the catalog in use, or `None` for msgids.
    pub fn language(&self) -> Option<&'static str> {
        self.catalog.map(|c| c.lang)
    }

    /// Translates `msgid`, or returns it unchanged when it has no translation.
    ///
    /// # Arguments
    /// * `msgid` - the English source string, as written in `po/*.po`
    pub fn tr(&self, msgid: &str) -> Cow<'static, str> {
        self.catalog
            .and_then(|c| {
                c.entries
                    .binary_search_by(|(id, _)| (*id).cmp(msgid))
                    .ok()
                    .map(|i| c.entries[i].1)
            })
            .map_or_else(|| Cow::Owned(msgid.to_string()), Cow::Borrowed)
    }

    /// Translates a message with a plural form chosen for `n`.
    ///
    /// Without a translation, returns `singular` when `n == 1` and `plural`
    /// otherwise (the source-language rule, as gettext does).
    ///
    /// # Arguments
    /// * `singular` - the English singular msgid
    /// * `plural` - the English plural msgid (`msgid_plural`)
    /// * `n` - the count the message describes
    pub fn trn(&self, singular: &str, plural: &str, n: u64) -> Cow<'static, str> {
        let translated = self.catalog.and_then(|c| {
            let i = c
                .plurals
                .binary_search_by(|(id, _)| (*id).cmp(singular))
                .ok()?;
            let form = c.plurals[i].1.get(c.rule.index(n))?;
            (!form.is_empty()).then_some(*form)
        });
        match translated {
            Some(text) => Cow::Borrowed(text),
            None if n == 1 => Cow::Owned(singular.to_string()),
            None => Cow::Owned(plural.to_string()),
        }
    }

    /// The plural form index this language uses for `n` (`0` is singular),
    /// from the catalog's `Plural-Forms` header.
    ///
    /// # Arguments
    /// * `n` - the count
    pub fn plural_form(&self, n: u64) -> usize {
        self.catalog.map_or(PluralRule::NotOne, |c| c.rule).index(n)
    }
}

/// Sets the process-wide language used by [`current`], [`tr!`](crate::tr)
/// and [`trn!`](crate::trn).
///
/// Only the translator changes; the process locale environment is left alone.
///
/// # Arguments
/// * `lang` - as for [`Translator::new`]
pub fn set_language(lang: &str) {
    CURRENT.store(catalog_index(lang).unwrap_or(usize::MAX), Ordering::Relaxed);
}

/// The language preference with `""` read as `"system"` (the two are the same preference).
///
/// # Arguments
/// * `lang` - a language preference
pub fn normalized_language(lang: &str) -> &str {
    if lang.is_empty() { "system" } else { lang }
}

/// True when switching from the `applied` language preference to `saved`
/// needs a restart (`""` and `"system"` are the same preference).
///
/// # Arguments
/// * `applied` - the preference in effect
/// * `saved` - the preference just saved
pub fn needs_restart(applied: &str, saved: &str) -> bool {
    normalized_language(saved) != normalized_language(applied)
}

/// The translator for the language last given to [`set_language`] (msgids
/// until it is called).
pub fn current() -> Translator {
    Translator {
        catalog: CATALOGS.get(CURRENT.load(Ordering::Relaxed)),
    }
}

/// The catalog for a language preference, resolving `"system"`.
fn catalog_index(lang: &str) -> Option<usize> {
    let lang = lang.trim();
    if lang.is_empty() || lang == "system" {
        return system_language().and_then(|l| find_catalog(&l));
    }
    find_catalog(lang)
}

/// Finds the catalog for a code or locale: `de`, `de_DE`, `de-DE`, `de_DE.UTF-8`.
fn find_catalog(locale: &str) -> Option<usize> {
    let code = locale
        .split(['_', '-', '.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    CATALOGS.iter().position(|c| c.lang == code)
}

/// The language the environment asks for, in gettext's order of precedence.
fn system_language() -> Option<String> {
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .find_map(|value| value.split(':').find(|l| !l.is_empty()).map(str::to_string))
}

/// Translates a string literal into the current language
/// ([`set_language`](crate::i18n::set_language)); evaluates to
/// `Cow<'static, str>`.
#[macro_export]
macro_rules! tr {
    ($msgid:expr) => {
        $crate::i18n::current().tr($msgid)
    };
}

/// Translates a message with a plural form chosen for `n`, in the current
/// language; evaluates to `Cow<'static, str>`.
#[macro_export]
macro_rules! trn {
    ($singular:expr, $plural:expr, $n:expr) => {
        $crate::i18n::current().trn($singular, $plural, $n)
    };
}
