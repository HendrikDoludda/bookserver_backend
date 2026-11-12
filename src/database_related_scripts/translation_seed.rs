//! The shipped translations: the files under `translations/`, compiled into the binary, checked,
//! and synced into the catalog tables at every start.

use std::collections::{BTreeMap, BTreeSet};

use once_cell::sync::Lazy;
use regex::Regex;
use rusqlite::{Connection, TransactionBehavior};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{
    database_related_scripts::{
        db::logged,
        db_translations::{self, StoredKey, SOURCE_LOCALE},
    },
    error_types::{DatabaseError, ShippedTranslationError},
    models::Audience,
};

// Writes each path once: as the file's name, and through `include_str!` as what is compiled in.
macro_rules! shipped {
    ($path:literal) => {
        ($path, include_str!(concat!("../../translations/", $path)))
    };
}

/// Every shipped file: its path under `translations/` and its contents. A test fails for a file
/// that is missing here.
const SHIPPED: &[(&str, &str)] = &[
    shipped!("en/language.json"),
    shipped!("en/frontend/errors.json"),
    shipped!("en/backend/email.json"),
    shipped!("de/language.json"),
    shipped!("de/frontend/errors.json"),
    shipped!("de/backend/email.json"),
];

static PLACEHOLDER: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{([a-z0-9_]+)\}").unwrap());

/// A language as its `language.json` describes it.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Language {
    pub display_name: String,
    pub sort_order: i64,
    pub icon: Option<String>,
}

/// A key: the namespace — the file it is in — and its dotted name inside the file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct KeyName {
    pub namespace: String,
    pub name: String,
}

impl KeyName {
    fn new(namespace: &str, name: String) -> Self {
        KeyName {
            namespace: namespace.to_string(),
            name,
        }
    }
}

/// A key as the English files define it, with its texts.
#[derive(Debug, PartialEq)]
pub(crate) struct Key {
    pub audience: Audience,
    pub hint: Option<String>,
    pub english: String,
    /// Its text in each other shipped language that has one, by language code.
    pub translated: BTreeMap<String, String>,
}

impl Key {
    /// Its text in the language `code`, if the shipped files have one.
    pub fn text(&self, code: &str) -> Option<&str> {
        if code == SOURCE_LOCALE {
            Some(&self.english)
        } else {
            self.translated.get(code).map(String::as_str)
        }
    }
}

/// What the shipped files say, once every check has passed.
#[derive(Debug, PartialEq)]
pub(crate) struct Catalog {
    pub languages: BTreeMap<String, Language>,
    pub keys: BTreeMap<KeyName, Key>,
}

/// What one sync changed.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct SyncSummary {
    pub languages_added: usize,
    pub keys_changed: usize,
    pub texts_changed: usize,
}

/// Checks the shipped files and syncs them into the catalog tables, in one transaction.
pub(crate) fn run(conn: &mut Connection) -> Result<(), DatabaseError> {
    let catalog = shipped().map_err(|err| {
        tracing::error!("The shipped translations are broken: {err}");
        DatabaseError::DatabaseInitializationFailure
    })?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(logged(
            "starting a transaction for",
            "the translation sync",
            DatabaseError::TransactionFailure,
        ))?;
    let summary = sync(&tx, &catalog)?;
    tx.commit().map_err(logged(
        "committing",
        "the translation sync",
        DatabaseError::OperationCommitFailure,
    ))?;
    tracing::info!(
        languages = catalog.languages.len(),
        keys = catalog.keys.len(),
        languages_added = summary.languages_added,
        keys_changed = summary.keys_changed,
        texts_changed = summary.texts_changed,
        "Synced the shipped translations"
    );
    Ok(())
}

/// The shipped files, read and checked.
pub(crate) fn shipped() -> Result<Catalog, ShippedTranslationError> {
    read(SHIPPED)
}

/// Brings the catalog tables in line with `catalog` and counts what that changed. Languages
/// only get added: a language's row belongs to the admins once it exists.
pub(crate) fn sync(conn: &Connection, catalog: &Catalog) -> Result<SyncSummary, DatabaseError> {
    let mut summary = SyncSummary::default();

    for (code, language) in &catalog.languages {
        let added = db_translations::insert_language_if_new(
            conn,
            code,
            &language.display_name,
            language.icon.as_deref(),
            language.sort_order,
        )?;
        summary.languages_added += usize::from(added);
    }
    db_translations::make_default_if_none(conn, SOURCE_LOCALE)?;

    let mut stored_keys: BTreeMap<KeyName, StoredKey> = db_translations::all_keys(conn)?
        .into_iter()
        .map(|key| (KeyName::new(&key.namespace, key.name.clone()), key))
        .collect();
    let mut stored_texts = BTreeMap::new();
    for code in catalog.languages.keys() {
        stored_texts.insert(code.as_str(), db_translations::texts_of(conn, code)?);
    }

    for (name, key) in &catalog.keys {
        let id = match stored_keys.remove(name) {
            Some(stored) => {
                if stored.audience != key.audience || stored.hint != key.hint {
                    db_translations::update_key(
                        conn,
                        stored.id,
                        key.audience,
                        key.hint.as_deref(),
                    )?;
                    summary.keys_changed += 1;
                }
                stored.id
            }
            None => {
                summary.keys_changed += 1;
                db_translations::insert_key(
                    conn,
                    &name.namespace,
                    &name.name,
                    key.audience,
                    key.hint.as_deref(),
                )?
            }
        };
        for (code, texts) in &mut stored_texts {
            match (texts.remove(&id), key.text(code)) {
                (Some(stored), Some(shipped)) if stored == shipped => {}
                (_, Some(shipped)) => {
                    db_translations::save_text(conn, code, id, shipped)?;
                    summary.texts_changed += 1;
                }
                (Some(_), None) => {
                    db_translations::delete_text(conn, code, id)?;
                    summary.texts_changed += 1;
                }
                (None, None) => {}
            }
        }
    }

    // Keys the English files no longer have; the cascade takes their texts with them.
    for stored in stored_keys.into_values() {
        db_translations::delete_key(conn, stored.id)?;
        summary.keys_changed += 1;
    }
    Ok(summary)
}

/// Whether `code` has the shape of a language tag: `ll` or `lll`, optionally followed by `-RR`
/// — `de`, `fil`, `pt-BR`.
pub(crate) fn is_language_code(code: &str) -> bool {
    let (language, region) = match code.split_once('-') {
        Some((language, region)) => (language, Some(region)),
        None => (code, None),
    };
    let language_fits =
        (2..=3).contains(&language.len()) && language.bytes().all(|b| b.is_ascii_lowercase());
    let region_fits = region
        .is_none_or(|region| region.len() == 2 && region.bytes().all(|b| b.is_ascii_uppercase()));
    language_fits && region_fits
}

/// What a path under `translations/` names.
enum FileKind<'a> {
    Language(&'a str),
    Namespace {
        code: &'a str,
        audience: Audience,
        namespace: &'a str,
    },
}

/// One namespace file: what its path says, and its texts and hints by dotted name.
struct NamespaceFile<'a> {
    file: &'a str,
    code: &'a str,
    audience: Audience,
    namespace: &'a str,
    texts: BTreeMap<String, String>,
    hints: BTreeMap<String, String>,
}

/// Reads and checks shipped files, given as (path under `translations/`, contents).
pub(crate) fn read(files: &[(&str, &str)]) -> Result<Catalog, ShippedTranslationError> {
    let mut languages = BTreeMap::new();
    let mut namespace_files = Vec::new();
    for &(file, contents) in files {
        match file_kind(file)? {
            FileKind::Language(code) => {
                let language =
                    serde_json::from_str(contents).map_err(|err| invalid_json(file, &err))?;
                languages.insert(code.to_string(), language);
            }
            FileKind::Namespace {
                code,
                audience,
                namespace,
            } => {
                let Value::Object(object) =
                    serde_json::from_str(contents).map_err(|err| invalid_json(file, &err))?
                else {
                    return Err(ShippedTranslationError::NotAnObject {
                        file: file.to_string(),
                    });
                };
                let mut parsed = NamespaceFile {
                    file,
                    code,
                    audience,
                    namespace,
                    texts: BTreeMap::new(),
                    hints: BTreeMap::new(),
                };
                flatten(&mut parsed, "", &object)?;
                namespace_files.push(parsed);
            }
        }
    }
    if !languages.contains_key(SOURCE_LOCALE) {
        return Err(missing_language_file(SOURCE_LOCALE));
    }
    if let Some(orphan) = namespace_files
        .iter()
        .find(|parsed| !languages.contains_key(parsed.code))
    {
        return Err(missing_language_file(orphan.code));
    }

    // The English files define the keys, and on which side each namespace is.
    let (english, translated): (Vec<_>, Vec<_>) = namespace_files
        .into_iter()
        .partition(|parsed| parsed.code == SOURCE_LOCALE);
    let mut sides = BTreeMap::new();
    let mut keys = BTreeMap::new();
    for parsed in english {
        if sides.insert(parsed.namespace, parsed.audience).is_some() {
            return Err(side_mismatch(&parsed));
        }
        let mut hints = parsed.hints;
        for (name, text) in parsed.texts {
            let key = Key {
                audience: parsed.audience,
                hint: hints.remove(&name),
                english: text,
                translated: BTreeMap::new(),
            };
            keys.insert(KeyName::new(parsed.namespace, name), key);
        }
    }

    for parsed in translated {
        if let Some(name) = parsed.hints.keys().next() {
            return Err(ShippedTranslationError::HintOutsideSource {
                file: parsed.file.to_string(),
                key: format!("@{name}"),
            });
        }
        if sides
            .get(parsed.namespace)
            .is_some_and(|side| *side != parsed.audience)
        {
            return Err(side_mismatch(&parsed));
        }
        for (name, text) in parsed.texts {
            let Some(key) = keys.get_mut(&KeyName::new(parsed.namespace, name.clone())) else {
                return Err(ShippedTranslationError::UnknownKey {
                    file: parsed.file.to_string(),
                    key: name,
                });
            };
            let expected = placeholders(&key.english);
            if placeholders(&text) != expected {
                return Err(ShippedTranslationError::PlaceholdersDiffer {
                    file: parsed.file.to_string(),
                    key: name,
                    expected: listed(&expected),
                });
            }
            key.translated.insert(parsed.code.to_string(), text);
        }
    }

    Ok(Catalog { languages, keys })
}

fn file_kind(file: &str) -> Result<FileKind<'_>, ShippedTranslationError> {
    let unexpected = || ShippedTranslationError::UnexpectedFile {
        file: file.to_string(),
    };
    let parts: Vec<&str> = file.split('/').collect();
    let (code, kind) = match parts[..] {
        [code, "language.json"] => (code, FileKind::Language(code)),
        [code, side, name] => {
            let audience = match side {
                "frontend" => Audience::Frontend,
                "backend" => Audience::Backend,
                _ => return Err(unexpected()),
            };
            let namespace = name.strip_suffix(".json").ok_or_else(unexpected)?;
            if !is_name(namespace) {
                return Err(ShippedTranslationError::BadName {
                    file: file.to_string(),
                    key: namespace.to_string(),
                });
            }
            let kind = FileKind::Namespace {
                code,
                audience,
                namespace,
            };
            (code, kind)
        }
        _ => return Err(unexpected()),
    };
    if !is_language_code(code) {
        return Err(ShippedTranslationError::BadLanguageCode {
            file: file.to_string(),
            code: code.to_string(),
        });
    }
    Ok(kind)
}

/// Collects the texts and `"@name"` hints of `object`, its nested objects as dotted names under
/// `prefix`.
fn flatten(
    parsed: &mut NamespaceFile<'_>,
    prefix: &str,
    object: &Map<String, Value>,
) -> Result<(), ShippedTranslationError> {
    for (entry, value) in object {
        let (is_hint, segment) = match entry.strip_prefix('@') {
            Some(segment) => (true, segment),
            None => (false, entry.as_str()),
        };
        let name = if prefix.is_empty() {
            segment.to_string()
        } else {
            format!("{prefix}.{segment}")
        };
        let file = parsed.file.to_string();
        let key = if is_hint {
            format!("@{name}")
        } else {
            name.clone()
        };
        if !is_name(segment) {
            return Err(ShippedTranslationError::BadName { file, key });
        }
        match value {
            Value::String(text) if text.trim().is_empty() => {
                return Err(ShippedTranslationError::EmptyText { file, key });
            }
            Value::String(hint) if is_hint => {
                if !matches!(object.get(segment), Some(Value::String(_))) {
                    return Err(ShippedTranslationError::HintWithoutText { file, key });
                }
                parsed.hints.insert(name, hint.clone());
            }
            Value::String(text) => {
                parsed.texts.insert(name, text.clone());
            }
            Value::Object(inner) if !is_hint => flatten(parsed, &name, inner)?,
            _ => return Err(ShippedTranslationError::NotText { file, key }),
        }
    }
    Ok(())
}

fn is_name(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn placeholders(text: &str) -> BTreeSet<&str> {
    PLACEHOLDER
        .captures_iter(text)
        .filter_map(|captures| captures.get(1))
        .map(|name| name.as_str())
        .collect()
}

/// The placeholders as a translator would write them: `{code}, {username}`.
fn listed(placeholders: &BTreeSet<&str>) -> String {
    if placeholders.is_empty() {
        return "none".to_string();
    }
    placeholders
        .iter()
        .map(|name| format!("{{{name}}}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn invalid_json(file: &str, err: &serde_json::Error) -> ShippedTranslationError {
    ShippedTranslationError::InvalidJson {
        file: file.to_string(),
        reason: err.to_string(),
    }
}

fn missing_language_file(code: &str) -> ShippedTranslationError {
    ShippedTranslationError::MissingLanguageFile {
        code: code.to_string(),
    }
}

fn side_mismatch(parsed: &NamespaceFile<'_>) -> ShippedTranslationError {
    ShippedTranslationError::SideMismatch {
        file: parsed.file.to_string(),
        namespace: parsed.namespace.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rusqlite::params;
    use walkdir::WalkDir;

    use super::*;
    use crate::test_support::migrated_connection;

    const ENGLISH: (&str, &str) = (
        "en/language.json",
        r#"{"display_name": "English", "sort_order": 1}"#,
    );
    const TEST_LANGUAGE: (&str, &str) = (
        "qaa/language.json",
        r#"{"display_name": "Test", "sort_order": 2}"#,
    );

    fn catalog(files: &[(&str, &str)]) -> Catalog {
        read(files).unwrap_or_else(|err| panic!("{err}"))
    }

    /// Every stored text as (language, `namespace.name`) → text.
    fn stored_texts(conn: &Connection) -> BTreeMap<(String, String), String> {
        conn.prepare(
            "SELECT t.locale_code, k.namespace || '.' || k.name, t.value \
             FROM translations t JOIN translation_keys k ON k.id = t.key_id",
        )
        .unwrap()
        .query_map([], |row| Ok(((row.get(0)?, row.get(1)?), row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    fn texts(expected: &[(&str, &str, &str)]) -> BTreeMap<(String, String), String> {
        expected
            .iter()
            .map(|(code, key, text)| ((code.to_string(), key.to_string()), text.to_string()))
            .collect()
    }

    #[test]
    fn the_shipped_files_pass_every_check() {
        let catalog = shipped().unwrap_or_else(|err| panic!("{err}"));
        let codes: Vec<&str> = catalog.languages.keys().map(String::as_str).collect();
        assert_eq!(codes, ["de", "en"]);
    }

    #[test]
    fn every_json_file_under_translations_is_shipped_and_every_shipped_file_exists() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("translations");
        let on_disk: BTreeSet<String> = WalkDir::new(&root)
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .map(|entry| {
                let relative = entry.path().strip_prefix(&root).unwrap();
                relative.to_string_lossy().replace('\\', "/")
            })
            .collect();
        let listed: BTreeSet<String> = SHIPPED.iter().map(|(path, _)| path.to_string()).collect();
        assert_eq!(listed, on_disk);
    }

    #[test]
    fn a_fresh_database_gets_the_shipped_languages_with_english_as_the_default() {
        let mut conn = migrated_connection();
        run(&mut conn).unwrap();

        let languages: Vec<(String, String, bool, bool)> = conn
            .prepare(
                "SELECT code, display_name, is_default, enabled FROM locales ORDER BY sort_order",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            languages,
            [
                ("en".to_string(), "English".to_string(), true, true),
                ("de".to_string(), "Deutsch".to_string(), false, true),
            ]
        );

        let keys = shipped().unwrap().keys.len();
        let english = stored_texts(&conn)
            .keys()
            .filter(|(code, _)| code == "en")
            .count();
        assert_eq!(english, keys, "every key has its English text");
    }

    #[test]
    fn a_second_sync_changes_nothing() {
        let conn = migrated_connection();
        let catalog = shipped().unwrap();

        let first = sync(&conn, &catalog).unwrap();
        assert_eq!(first.languages_added, 2);
        assert_eq!(first.keys_changed, catalog.keys.len());

        assert_eq!(sync(&conn, &catalog).unwrap(), SyncSummary::default());
    }

    #[test]
    fn keys_and_texts_follow_the_files() {
        let conn = migrated_connection();
        let before = catalog(&[
            ENGLISH,
            TEST_LANGUAGE,
            (
                "en/frontend/test.json",
                r#"{"kept": "Kept", "changed": "Old", "removed": "Removed"}"#,
            ),
            (
                "qaa/frontend/test.json",
                r#"{"kept": "Kept in qaa", "changed": "Old in qaa", "removed": "Removed in qaa"}"#,
            ),
        ]);
        sync(&conn, &before).unwrap();

        let after = catalog(&[
            ENGLISH,
            TEST_LANGUAGE,
            (
                "en/frontend/test.json",
                r#"{"kept": "Kept", "changed": "New", "added": "Added"}"#,
            ),
            ("qaa/frontend/test.json", r#"{"changed": "New in qaa"}"#),
        ]);
        let summary = sync(&conn, &after).unwrap();

        assert_eq!(
            stored_texts(&conn),
            texts(&[
                ("en", "test.added", "Added"),
                ("en", "test.changed", "New"),
                ("en", "test.kept", "Kept"),
                ("qaa", "test.changed", "New in qaa"),
            ]),
            "qaa's 'kept' is gone, so it falls back to English"
        );
        assert_eq!(
            summary,
            SyncSummary {
                languages_added: 0,
                keys_changed: 2,
                texts_changed: 4,
            },
            "keys: 'added' and 'removed'; texts: 'added', both 'changed' and qaa's 'kept'"
        );
    }

    #[test]
    fn a_new_hint_or_side_updates_the_key_and_keeps_its_id() {
        let conn = migrated_connection();
        sync(
            &conn,
            &catalog(&[ENGLISH, ("en/frontend/test.json", r#"{"title": "Title"}"#)]),
        )
        .unwrap();
        let before = db_translations::all_keys(&conn).unwrap();

        sync(
            &conn,
            &catalog(&[
                ENGLISH,
                (
                    "en/backend/test.json",
                    r#"{"title": "Title", "@title": "A hint"}"#,
                ),
            ]),
        )
        .unwrap();

        let after = db_translations::all_keys(&conn).unwrap();
        assert_eq!(
            after,
            [StoredKey {
                id: before[0].id,
                namespace: "test".to_string(),
                name: "title".to_string(),
                audience: Audience::Backend,
                hint: Some("A hint".to_string()),
            }]
        );
    }

    #[test]
    fn rows_an_admin_owns_survive_a_sync() {
        let conn = migrated_connection();
        let files = [
            ENGLISH,
            TEST_LANGUAGE,
            ("en/frontend/test.json", r#"{"title": "Title"}"#),
        ];
        sync(&conn, &catalog(&files)).unwrap();
        conn.execute_batch(
            "UPDATE locales SET display_name = 'Mine', enabled = FALSE WHERE code = 'qaa';
             UPDATE locales SET is_default = FALSE WHERE code = 'en';
             INSERT INTO locales (code, display_name, is_default, sort_order)
                 VALUES ('qab', 'Added by an admin', TRUE, 3);",
        )
        .unwrap();
        let key: i64 = conn
            .query_row("SELECT id FROM translation_keys", [], |row| row.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO translations (locale_code, key_id, value) VALUES ('qab', ?1, 'Mine')",
            params![key],
        )
        .unwrap();

        sync(&conn, &catalog(&files)).unwrap();

        let languages: Vec<(String, String, bool, bool)> = conn
            .prepare("SELECT code, display_name, is_default, enabled FROM locales ORDER BY code")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            languages,
            [
                ("en".to_string(), "English".to_string(), false, true),
                ("qaa".to_string(), "Mine".to_string(), false, false),
                (
                    "qab".to_string(),
                    "Added by an admin".to_string(),
                    true,
                    true
                ),
            ],
            "no language row is changed, and English doesn't take the default back"
        );
        assert_eq!(
            stored_texts(&conn).get(&("qab".to_string(), "test.title".to_string())),
            Some(&"Mine".to_string()),
            "a language the files don't ship keeps its texts"
        );
    }

    #[test]
    fn nested_objects_become_dotted_names_and_hints_attach_to_their_text() {
        let read = catalog(&[
            ENGLISH,
            (
                "en/frontend/library.json",
                r#"{"unread": {"one": "{count} unread book", "other": "{count} unread books",
                    "@other": "More than one"}}"#,
            ),
        ]);
        let names: Vec<(&str, Option<&str>)> = read
            .keys
            .iter()
            .map(|(key, value)| (key.name.as_str(), value.hint.as_deref()))
            .collect();
        assert_eq!(
            names,
            [
                ("unread.one", None),
                ("unread.other", Some("More than one"))
            ]
        );
    }

    #[test]
    fn a_translation_may_leave_keys_out_and_reorder_placeholders() {
        let read = catalog(&[
            ENGLISH,
            TEST_LANGUAGE,
            (
                "en/backend/mail.json",
                r#"{"body": "Hello {username}, {code}", "subject": "Code"}"#,
            ),
            (
                "qaa/backend/mail.json",
                r#"{"body": "{code} for {username}"}"#,
            ),
        ]);
        let body = &read.keys[&KeyName::new("mail", "body".to_string())];
        assert_eq!(body.text("qaa"), Some("{code} for {username}"));
        let subject = &read.keys[&KeyName::new("mail", "subject".to_string())];
        assert_eq!(subject.text("qaa"), None);
        assert_eq!(subject.text("en"), Some("Code"));
    }

    #[test]
    fn each_rule_refuses_a_file_that_breaks_it() {
        use ShippedTranslationError as E;
        let english_errors = (
            "en/frontend/errors.json",
            r#"{"not_found": "Not found", "mail": {"body": "Hi {username}: {code}"}}"#,
        );
        let file = |path: &str| path.to_string();
        let cases: Vec<(Vec<(&str, &str)>, E)> = vec![
            (
                vec![ENGLISH, ("en/errors.json", "{}")],
                E::UnexpectedFile {
                    file: file("en/errors.json"),
                },
            ),
            (
                vec![ENGLISH, ("en/frontend/errors.txt", "{}")],
                E::UnexpectedFile {
                    file: file("en/frontend/errors.txt"),
                },
            ),
            (
                vec![ENGLISH, ("en/admin/errors.json", "{}")],
                E::UnexpectedFile {
                    file: file("en/admin/errors.json"),
                },
            ),
            (
                vec![ENGLISH, ("German/language.json", "{}")],
                E::BadLanguageCode {
                    file: file("German/language.json"),
                    code: "German".to_string(),
                },
            ),
            (
                vec![ENGLISH, ("en/frontend/errors.json", "[]")],
                E::NotAnObject {
                    file: file("en/frontend/errors.json"),
                },
            ),
            (
                vec![ENGLISH, ("en/frontend/errors.json", r#"{"count": 3}"#)],
                E::NotText {
                    file: file("en/frontend/errors.json"),
                    key: "count".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    ("en/frontend/errors.json", r#"{"a": "A", "@a": {"b": "B"}}"#),
                ],
                E::NotText {
                    file: file("en/frontend/errors.json"),
                    key: "@a".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    ("en/frontend/errors.json", r#"{"Not-Found": "x"}"#),
                ],
                E::BadName {
                    file: file("en/frontend/errors.json"),
                    key: "Not-Found".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    ("en/frontend/errors.json", r#"{"a": {"b.c": "x"}}"#),
                ],
                E::BadName {
                    file: file("en/frontend/errors.json"),
                    key: "a.b.c".to_string(),
                },
            ),
            (
                vec![ENGLISH, ("en/frontend/my-errors.json", "{}")],
                E::BadName {
                    file: file("en/frontend/my-errors.json"),
                    key: "my-errors".to_string(),
                },
            ),
            (
                vec![ENGLISH, ("en/frontend/errors.json", r#"{"a": " "}"#)],
                E::EmptyText {
                    file: file("en/frontend/errors.json"),
                    key: "a".to_string(),
                },
            ),
            (
                vec![ENGLISH, ("en/frontend/errors.json", r#"{"@a": "A hint"}"#)],
                E::HintWithoutText {
                    file: file("en/frontend/errors.json"),
                    key: "@a".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    TEST_LANGUAGE,
                    english_errors,
                    (
                        "qaa/frontend/errors.json",
                        r#"{"not_found": "x", "@not_found": "y"}"#,
                    ),
                ],
                E::HintOutsideSource {
                    file: file("qaa/frontend/errors.json"),
                    key: "@not_found".to_string(),
                },
            ),
            (
                vec![english_errors],
                E::MissingLanguageFile {
                    code: "en".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    english_errors,
                    ("qaa/frontend/errors.json", r#"{"not_found": "x"}"#),
                ],
                E::MissingLanguageFile {
                    code: "qaa".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    english_errors,
                    ("en/backend/errors.json", r#"{"other": "x"}"#),
                ],
                E::SideMismatch {
                    file: file("en/backend/errors.json"),
                    namespace: "errors".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    TEST_LANGUAGE,
                    english_errors,
                    ("qaa/backend/errors.json", r#"{"not_found": "x"}"#),
                ],
                E::SideMismatch {
                    file: file("qaa/backend/errors.json"),
                    namespace: "errors".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    TEST_LANGUAGE,
                    english_errors,
                    ("qaa/frontend/errors.json", r#"{"not_fuond": "x"}"#),
                ],
                E::UnknownKey {
                    file: file("qaa/frontend/errors.json"),
                    key: "not_fuond".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    TEST_LANGUAGE,
                    english_errors,
                    (
                        "qaa/frontend/errors.json",
                        r#"{"mail": {"body": "Hallo {username}"}}"#,
                    ),
                ],
                E::PlaceholdersDiffer {
                    file: file("qaa/frontend/errors.json"),
                    key: "mail.body".to_string(),
                    expected: "{code}, {username}".to_string(),
                },
            ),
            (
                vec![
                    ENGLISH,
                    TEST_LANGUAGE,
                    english_errors,
                    (
                        "qaa/frontend/errors.json",
                        r#"{"not_found": "{code} fehlt"}"#,
                    ),
                ],
                E::PlaceholdersDiffer {
                    file: file("qaa/frontend/errors.json"),
                    key: "not_found".to_string(),
                    expected: "none".to_string(),
                },
            ),
        ];

        for (files, expected) in cases {
            assert_eq!(read(&files), Err(expected));
        }
    }

    #[test]
    fn a_file_that_isnt_json_is_refused_with_the_parsers_reason() {
        let result = read(&[ENGLISH, ("en/frontend/errors.json", r#"{"a": "#)]);
        assert!(
            matches!(
                &result,
                Err(ShippedTranslationError::InvalidJson { file, reason })
                    if file == "en/frontend/errors.json" && reason.contains("line 1")
            ),
            "{result:?}"
        );
        let result = read(&[("en/language.json", r#"{"display_name": "English"}"#)]);
        assert!(
            matches!(
                &result,
                Err(ShippedTranslationError::InvalidJson { reason, .. })
                    if reason.contains("sort_order")
            ),
            "a missing field is named: {result:?}"
        );
    }

    #[test]
    fn a_language_code_is_two_or_three_lowercase_letters_and_maybe_a_region_in_capitals() {
        for good in ["de", "en", "fil", "pt-BR", "yue-HK"] {
            assert!(is_language_code(good), "{good:?}");
        }
        for bad in [
            "", "d", "deut", "DE", "De", "de-at", "de-A", "de-AUT", "de_AT", "de-", "-AT",
            "de-AT-x", "zh-Hant", "es-419", " de", "de ", "dé",
        ] {
            assert!(!is_language_code(bad), "{bad:?}");
        }
    }
}
