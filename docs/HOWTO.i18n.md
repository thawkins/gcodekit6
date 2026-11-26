# HOWTO: i18n / Translations

GCodeKit5 reads its translations from gettext `.po` files, but it does not use gettext at run time. `build.rs` parses `po/*.po` and embeds the catalogs in the binary. You do not need `msgfmt`, `.mo` files or a gettext library to build or run the app.

## Where translations live

- Template (source strings): `po/gcodekit5.pot`
- Per-language translations: `po/<lang>.po` (e.g. `po/fr.po`)
- Build-time embedding: `crates/gcodekit5-settings/build.rs` (parses the catalogs with `polib`)
- Runtime lookup: `crates/gcodekit5-settings/src/i18n.rs` (re-exported as `gcodekit5_app::i18n`)
- Extraction/update script: `scripts/update-po.sh`

## How strings are translated in code

In the egui front end, translate through the app's `Translator`:

```rust
use gcodekit5_app::i18n::Translator;

let label = translator.tr("Machine Control");
let lines = translator.trn("line", "lines", n);
```

Model code in `gcodekit5-app` and `gcodekit5-settings` has no translator at hand. It uses the `tr!` and `trn!` macros, which read the process-wide current language:

```rust
let reason = crate::tr!("the machine must be idle");
```

To mark a literal for extraction without translating it where it is written (a `match` arm that is translated later through a variable), wrap it in `n_!("...")`.

Notes:
- The msgid is the English source string and must match the `.po` entry exactly.
- Avoid embedding `\n` or other control characters unless you really want them in the UI.

## Updating translations after UI changes

When you add or change strings, update the POT and merge the changes into all PO files:

```bash
./scripts/update-po.sh
```

This runs `xgettext --keyword=tr --keyword=trn:1,2` (plus the `t!`, `n_!` and `gettext` keywords) over the tracked Rust sources, then `msgmerge` and `msginit` for the configured languages.

### Requirements

Only the maintainer's extraction script needs the gettext tools (`xgettext`, `msgmerge`, `msginit`). Building and running the app does not. On Fedora:

```bash
sudo dnf install gettext
```

## Editing translations

Open the appropriate `po/<lang>.po` and translate the `msgstr` values.

Example:

```po
msgid "Visualizer"
msgstr "Visualiseur"
```

Untranslated and fuzzy entries are skipped when the catalogs are embedded, so those strings show in English.

## Adding a new language

1) Add the language code to `scripts/update-po.sh`:

Location:
- `scripts/update-po.sh` → `LANGS=("fr" "de" "es" "pt" "it")`

Add your new language (example `nl`):

```bash
LANGS=("fr" "de" "es" "pt" "it" "nl")
```

2) Generate/init the new PO:

```bash
./scripts/update-po.sh
```

This will create `po/nl.po` if it doesn't exist.

3) Translate `po/nl.po`.

4) If the language uses a plural rule other than `n != 1` or `n > 1`, add it to `build.rs` and `PluralRule` in `i18n.rs`. The build fails on an unknown `Plural-Forms` rule.

## Running the app in a specific language

Set the language in Preferences; a change takes effect after a restart. With the language set to "system", the app uses the first of `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and `LANG` that names a language, so you can also force one from the environment:

```bash
LANGUAGE=fr cargo run
```

## Troubleshooting

### Strings not translating

Checklist:
- The string goes through `tr(...)`, `trn(...)`, `tr!(...)`, `trn!(...)` or `n_!(...)`.
- `po/gcodekit5.pot` contains the `msgid` (run `./scripts/update-po.sh`).
- Your `po/<lang>.po` has a non-empty, non-fuzzy `msgstr`.
- You rebuilt after editing the `.po` file (`build.rs` re-embeds the catalogs).
- The language is set in Preferences, or `LANGUAGE=<lang>` is set.
