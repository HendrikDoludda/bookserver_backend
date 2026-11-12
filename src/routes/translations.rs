//! The translation catalog as the apps read it: the languages on offer, and a language's texts,
//! each falling back to English.

use std::{collections::BTreeSet, sync::Arc};

use crate::{
    data_models::translations_model::{Locale, Translations},
    database_related_scripts::db_translations,
    db::Database,
    error_types::{AppErrors, TranslationError},
};

/// The languages the server offers, in their order.
pub async fn locales(db: Arc<Database>) -> Result<Vec<Locale>, AppErrors> {
    db.read(|conn| Ok(db_translations::enabled_locales(conn)?))
        .await
}

/// The apps' texts in the language `code`, by namespace; a text the language lacks is the
/// English one. `sections`, namespaces separated by commas, limits them.
pub async fn translations(
    db: Arc<Database>,
    code: String,
    sections: Option<String>,
) -> Result<Translations, AppErrors> {
    let wanted = requested_sections(sections.as_deref());
    let texts = db
        .read(move |conn| {
            if !db_translations::is_enabled(conn, &code)? {
                return Err(TranslationError::UnknownLocale.into());
            }
            Ok(db_translations::frontend_texts(conn, &code)?)
        })
        .await?;

    let mut translations = Translations::new();
    for (namespace, name, text) in texts {
        if wanted
            .as_ref()
            .is_none_or(|wanted| wanted.contains(&namespace))
        {
            translations
                .entry(namespace)
                .or_default()
                .insert(name, text);
        }
    }
    Ok(translations)
}

/// The namespaces `?sections=` names; `None`, meaning all of them, while it names none.
fn requested_sections(sections: Option<&str>) -> Option<BTreeSet<String>> {
    let wanted: BTreeSet<String> = sections?
        .split(',')
        .map(str::trim)
        .filter(|section| !section.is_empty())
        .map(str::to_string)
        .collect();
    (!wanted.is_empty()).then_some(wanted)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::test_support::{insert_language, insert_texts, TempDatabase};

    fn set_up(temp: &TempDatabase, setup: impl FnOnce(&Connection)) {
        temp.db
            .setup_new_transaction(|conn| {
                setup(conn);
                Ok(())
            })
            .unwrap();
    }

    async fn texts_of(temp: &TempDatabase, code: &str, sections: Option<&str>) -> Translations {
        translations(
            Arc::clone(&temp.db),
            code.to_string(),
            sections.map(str::to_string),
        )
        .await
        .unwrap()
    }

    fn namespaces(translations: Translations) -> Vec<String> {
        translations.into_keys().collect()
    }

    #[tokio::test]
    async fn the_locales_are_the_enabled_ones_in_their_order() {
        let temp = TempDatabase::open();
        set_up(&temp, |conn| {
            insert_language(conn, "qaa", 0);
            conn.execute_batch(
                "UPDATE locales SET icon = '📘' WHERE code = 'qaa';
                 UPDATE locales SET enabled = FALSE WHERE code = 'de';",
            )
            .unwrap();
        });

        let listed = locales(Arc::clone(&temp.db)).await.unwrap();

        assert_eq!(
            listed,
            [
                Locale {
                    code: "qaa".to_string(),
                    display_name: "qaa".to_string(),
                    icon: Some("📘".to_string()),
                    is_default: false,
                },
                Locale {
                    code: "en".to_string(),
                    display_name: "English".to_string(),
                    icon: None,
                    is_default: true,
                },
            ]
        );
    }

    #[tokio::test]
    async fn a_missing_text_falls_back_to_english_key_by_key() {
        let temp = TempDatabase::open();
        set_up(&temp, |conn| {
            insert_language(conn, "qaa", 3);
            insert_texts(
                conn,
                "frontend",
                "test",
                "translated",
                &[("en", "English"), ("qaa", "Translated")],
            );
            insert_texts(
                conn,
                "frontend",
                "test",
                "missing",
                &[("en", "Only English")],
            );
        });

        let texts = texts_of(&temp, "qaa", Some("test")).await;

        let test = &texts["test"];
        assert_eq!(test["translated"], "Translated");
        assert_eq!(test["missing"], "Only English");
        assert_eq!(test.len(), 2);
    }

    #[tokio::test]
    async fn backend_texts_never_reach_the_apps() {
        let temp = TempDatabase::open();
        set_up(&temp, |conn| {
            insert_texts(
                conn,
                "backend",
                "test",
                "server_only",
                &[("en", "Only the server renders this")],
            );
        });

        for code in ["en", "de"] {
            let texts = texts_of(&temp, code, None).await;
            assert!(!texts.contains_key("test"), "{code}");
            assert!(
                !texts.contains_key("email"),
                "{code}: the mail texts are backend texts"
            );
            assert!(texts.contains_key("errors"), "{code}");
        }
    }

    #[tokio::test]
    async fn sections_limit_the_answer_to_the_namespaces_named() {
        let temp = TempDatabase::open();
        set_up(&temp, |conn| {
            insert_texts(conn, "frontend", "first", "a", &[("en", "A")]);
            insert_texts(conn, "frontend", "second", "b", &[("en", "B")]);
        });

        assert_eq!(
            namespaces(texts_of(&temp, "en", Some("first")).await),
            ["first"]
        );
        assert_eq!(
            namespaces(texts_of(&temp, "en", Some(" second , first ")).await),
            ["first", "second"]
        );
        assert_eq!(
            namespaces(texts_of(&temp, "en", Some("first,nope")).await),
            ["first"],
            "an unknown name matches nothing"
        );
        assert!(texts_of(&temp, "en", Some("nope")).await.is_empty());

        let all = namespaces(texts_of(&temp, "en", None).await);
        for section in ["first", "second", "errors"] {
            assert!(all.contains(&section.to_string()), "{section}");
        }
        for empty in ["", ",", " , "] {
            assert_eq!(
                namespaces(texts_of(&temp, "en", Some(empty)).await),
                all,
                "{empty:?} names nothing, so it means all"
            );
        }
    }

    #[tokio::test]
    async fn a_language_the_server_doesnt_offer_is_refused() {
        let temp = TempDatabase::open();
        set_up(&temp, |conn| {
            conn.execute("UPDATE locales SET enabled = FALSE WHERE code = 'de'", [])
                .unwrap();
        });

        for code in ["fr", "de", "EN", ""] {
            let result = translations(Arc::clone(&temp.db), code.to_string(), None).await;
            assert!(
                matches!(
                    result,
                    Err(AppErrors::Translation(TranslationError::UnknownLocale))
                ),
                "{code:?}"
            );
        }
    }
}
