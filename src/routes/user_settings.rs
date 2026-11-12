//! A user's own settings: reading them, and replacing them once each value passes its check.

use std::sync::Arc;

use rusqlite::Connection;

use crate::{
    data_models::settings_model::Settings,
    database_related_scripts::{db_translations, extract::Extract},
    db::Database,
    error_types::{AppErrors, DatabaseError, SettingsError},
    models::{UserSettings, UserSettingsDatabaseColumns},
};

/// The user's settings, or the defaults while they have changed none.
pub async fn read_settings(db: Arc<Database>, user_id: i64) -> Result<Settings, AppErrors> {
    let stored = db.read(move |conn| Ok(find(conn, user_id)?)).await?;
    Ok(stored.map(Settings::from).unwrap_or_default())
}

/// Replaces the user's settings and answers them as stored; the first change creates the row.
pub async fn replace_settings(
    db: Arc<Database>,
    user_id: i64,
    settings: Settings,
) -> Result<Settings, AppErrors> {
    let row = UserSettings {
        user_id,
        locale: settings.locale,
    };
    db.transact(move |conn| {
        if let Some(locale) = &row.locale {
            check_locale(conn, locale)?;
        }
        // Inside the write lock, so of two first changes at once the second sees the first one's
        // row: an update, never the duplicate key's 500.
        if find(conn, user_id)?.is_some() {
            Database::update_value_with_connection::<UserSettings>(
                conn,
                UserSettingsDatabaseColumns::Locale,
                &row.locale,
                user_id,
            )?;
        } else {
            Database::insert_with_connection(conn, &row)?;
        }
        Ok(Settings::from(row))
    })
    .await
}

/// 400 `invalid_locale` unless the server offers the language `locale`.
pub(crate) fn check_locale(conn: &Connection, locale: &str) -> Result<(), AppErrors> {
    if db_translations::is_enabled(conn, locale)? {
        Ok(())
    } else {
        Err(SettingsError::InvalidLocale.into())
    }
}

/// The user's row; `None` while they have changed nothing.
fn find(conn: &Connection, user_id: i64) -> Result<Option<UserSettings>, DatabaseError> {
    // Not `Database::get_entry`, which makes a missing row an error: here it is the usual case.
    UserSettings::extract(conn, user_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_user, TempDatabase};

    fn add_user(temp: &TempDatabase, username: &str) -> i64 {
        temp.db
            .setup_new_transaction(|conn| Ok(insert_user(conn, username)))
            .unwrap()
    }

    async fn change(
        temp: &TempDatabase,
        user_id: i64,
        locale: Option<&str>,
    ) -> Result<Settings, AppErrors> {
        let settings = Settings {
            locale: locale.map(str::to_string),
        };
        replace_settings(Arc::clone(&temp.db), user_id, settings).await
    }

    async fn locale_of(temp: &TempDatabase, user_id: i64) -> Option<String> {
        read_settings(Arc::clone(&temp.db), user_id)
            .await
            .unwrap()
            .locale
    }

    fn rows(temp: &TempDatabase) -> i64 {
        temp.db
            .setup_new_transaction(|conn| {
                Ok(conn
                    .query_row("SELECT COUNT(*) FROM user_settings", [], |row| row.get(0))
                    .unwrap())
            })
            .unwrap()
    }

    #[tokio::test]
    async fn without_a_row_the_answer_is_the_defaults_and_reading_writes_nothing() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice");

        let settings = read_settings(Arc::clone(&temp.db), alice).await.unwrap();

        assert_eq!(settings, Settings { locale: None });
        assert_eq!(rows(&temp), 0);
    }

    #[tokio::test]
    async fn a_change_reads_back_and_later_changes_and_a_reset_update_the_same_row() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice");

        let answer = change(&temp, alice, Some("de")).await.unwrap();
        assert_eq!(
            answer.locale.as_deref(),
            Some("de"),
            "the answer is what was stored"
        );
        assert_eq!(locale_of(&temp, alice).await.as_deref(), Some("de"));

        change(&temp, alice, Some("en")).await.unwrap();
        assert_eq!(locale_of(&temp, alice).await.as_deref(), Some("en"));

        let reset = change(&temp, alice, None).await.unwrap();
        assert_eq!(reset.locale, None);
        assert_eq!(locale_of(&temp, alice).await, None);
        assert_eq!(
            rows(&temp),
            1,
            "every change after the first updates its row"
        );
    }

    fn refused(result: Result<Settings, AppErrors>) -> bool {
        matches!(
            result,
            Err(AppErrors::Settings(SettingsError::InvalidLocale))
        )
    }

    #[tokio::test]
    async fn a_language_the_server_doesnt_offer_is_refused_and_changes_nothing() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice");
        change(&temp, alice, Some("en")).await.unwrap();

        for bad in ["fr", "pt-BR", "german", "", "DE", "de_AT"] {
            assert!(refused(change(&temp, alice, Some(bad)).await), "{bad:?}");
        }
        assert_eq!(locale_of(&temp, alice).await.as_deref(), Some("en"));
    }

    #[tokio::test]
    async fn a_disabled_language_is_refused() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice");
        temp.db
            .setup_new_transaction(|conn| {
                conn.execute("UPDATE locales SET enabled = FALSE WHERE code = 'de'", [])
                    .unwrap();
                Ok(())
            })
            .unwrap();

        assert!(refused(change(&temp, alice, Some("de")).await));
        assert_eq!(locale_of(&temp, alice).await, None);
    }

    #[tokio::test]
    async fn users_do_not_see_each_others_settings() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice");
        let bob = add_user(&temp, "bob");

        change(&temp, alice, Some("de")).await.unwrap();
        assert_eq!(locale_of(&temp, bob).await, None);

        change(&temp, bob, Some("en")).await.unwrap();
        assert_eq!(locale_of(&temp, alice).await.as_deref(), Some("de"));
        assert_eq!(locale_of(&temp, bob).await.as_deref(), Some("en"));
    }

    #[tokio::test]
    async fn two_first_changes_at_once_both_succeed_on_one_row() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice");

        let (a, b) = tokio::join!(
            change(&temp, alice, Some("de")),
            change(&temp, alice, Some("en")),
        );

        assert!(a.is_ok() && b.is_ok(), "never the duplicate key's 500");
        assert_eq!(rows(&temp), 1);
        let stored = locale_of(&temp, alice).await;
        assert!(matches!(stored.as_deref(), Some("de" | "en")), "{stored:?}");
    }
}
