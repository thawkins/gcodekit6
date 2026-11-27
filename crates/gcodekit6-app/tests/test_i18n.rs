//! Tests for `gcodekit6_app::i18n`: translations embedded from `po/*.po` at
//! build time, looked up without gettext.
#![allow(unsafe_code)]

use gcodekit6_app::i18n::{self, Translator};

#[test]
fn test_po_lookup_de() {
    // po/de.po: msgid "Connect" / msgstr "Verbinden"
    assert_eq!(Translator::new("de").tr("Connect"), "Verbinden");
    // Region and encoding suffixes select the same catalog.
    assert_eq!(Translator::new("de_DE.UTF-8").tr("Connect"), "Verbinden");
    assert_eq!(Translator::new("fr").tr("Connect"), "Connecter");
}

#[test]
fn test_missing_falls_back_to_msgid() {
    let de = Translator::new("de");
    assert_eq!(
        de.tr("No such msgid in any catalog"),
        "No such msgid in any catalog"
    );
    // A language without a catalog (English is the source language) returns msgids.
    assert_eq!(Translator::new("en").tr("Connect"), "Connect");
    assert_eq!(Translator::new("xx").tr("Connect"), "Connect");
}

#[test]
fn test_plural_forms() {
    // po/fr.po: plural=(n > 1); po/de.po: plural=(n != 1); po/es.po has no
    // Plural-Forms header, so the gettext default (n != 1) applies.
    let fr = Translator::new("fr");
    assert_eq!(
        [0, 1, 2].map(|n| fr.plural_form(n)),
        [0, 0, 1],
        "fr: 0 and 1 are singular"
    );
    let de = Translator::new("de");
    assert_eq!([0, 1, 2].map(|n| de.plural_form(n)), [1, 0, 1]);
    let es = Translator::new("es");
    assert_eq!([0, 1, 2].map(|n| es.plural_form(n)), [1, 0, 1]);

    // No catalog entry: the msgids are chosen by the source-language rule.
    assert_eq!(fr.trn("line", "lines", 1), "line");
    assert_eq!(fr.trn("line", "lines", 0), "lines");
    assert_eq!(fr.trn("line", "lines", 5), "lines");
}

#[test]
fn test_tr_macro_uses_current_language() {
    let _env = crate::global_env_lock();
    i18n::set_language("de");
    assert_eq!(gcodekit6_app::tr!("Connect"), "Verbinden");
    assert_eq!(i18n::current().tr("Connect"), "Verbinden");
    i18n::set_language("en");
    assert_eq!(gcodekit6_app::tr!("Connect"), "Connect");
    assert_eq!(gcodekit6_app::trn!("line", "lines", 2), "lines");
}

/// Counterpart of the GTK `test_i18n_locale_env` guard: choosing a language
/// for the Translator never rewrites the process locale variables.
#[test]
fn test_set_language_leaves_env_untouched() {
    let _env = crate::global_env_lock();
    // SAFETY: the binary-wide env lock is held and this test starts no threads.
    unsafe {
        std::env::set_var("LANG", "en_US.UTF-8");
        std::env::remove_var("LANGUAGE");
    }

    i18n::set_language("fr");

    assert_eq!(std::env::var("LANG").as_deref(), Ok("en_US.UTF-8"));
    assert!(std::env::var_os("LANGUAGE").is_none());
    i18n::set_language("en");
}

#[test]
fn test_system_language_reads_env() {
    let _env = crate::global_env_lock();
    // SAFETY: the binary-wide env lock is held and this test starts no threads.
    unsafe {
        std::env::set_var("LANGUAGE", "it:en");
    }
    assert_eq!(Translator::new("system").tr("Connect"), "Connetti");
    // SAFETY: as above.
    unsafe {
        std::env::remove_var("LANGUAGE");
    }
}
