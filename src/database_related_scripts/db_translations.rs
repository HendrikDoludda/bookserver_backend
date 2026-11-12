//! The translation catalog's queries — `locales`, `translation_keys` and the `translations`
//! junction — and how an `Audience` is stored. `translation_keys` has an integer key, but it is
//! only ever read and written together with the other two, so its queries live here as well.

use std::collections::BTreeMap;

use rusqlite::{
    params,
    types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
    Connection, ToSql,
};

use crate::{database_related_scripts::db::logged, error_types::DatabaseError, models::Audience};

/// The language every key is written in first; a text missing in another language falls back
/// to it.
pub const SOURCE_LOCALE: &str = "en";

impl ToSql for Audience {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for Audience {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "frontend" => Ok(Audience::Frontend),
            "backend" => Ok(Audience::Backend),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

/// One row of `translation_keys`.
#[derive(Debug, PartialEq)]
pub struct StoredKey {
    pub id: i64,
    pub namespace: String,
    pub name: String,
    pub audience: Audience,
    pub hint: Option<String>,
}

/// Adds a language unless one with this code exists, and answers whether it did; an existing
/// row keeps what it has.
pub fn insert_language_if_new(
    conn: &Connection,
    code: &str,
    display_name: &str,
    icon: Option<&str>,
    sort_order: i64,
) -> Result<bool, DatabaseError> {
    conn.execute(
        "INSERT INTO locales (code, display_name, icon, sort_order) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT (code) DO NOTHING",
        params![code, display_name, icon, sort_order],
    )
    .map(|inserted| inserted > 0)
    .map_err(logged(
        "writing",
        "locales",
        DatabaseError::InsertionFailure,
    ))
}

/// Makes `code` the default language while there is none, if it is enabled.
pub fn make_default_if_none(conn: &Connection, code: &str) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE locales SET is_default = TRUE \
         WHERE code = ?1 AND enabled AND NOT EXISTS (SELECT 1 FROM locales WHERE is_default)",
        [code],
    )
    .map_err(logged("writing", "locales", DatabaseError::UpdatingFailure))?;
    Ok(())
}

/// Every key, in no particular order.
pub fn all_keys(conn: &Connection) -> Result<Vec<StoredKey>, DatabaseError> {
    let mut stmt = conn
        .prepare("SELECT id, namespace, name, audience, hint FROM translation_keys")
        .map_err(logged(
            "reading",
            "translation_keys",
            DatabaseError::TaskPreparationFailure,
        ))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(StoredKey {
                id: row.get("id")?,
                namespace: row.get("namespace")?,
                name: row.get("name")?,
                audience: row.get("audience")?,
                hint: row.get("hint")?,
            })
        })
        .map_err(logged(
            "reading",
            "translation_keys",
            DatabaseError::QueryFailure,
        ))?;
    rows.collect::<Result<_, _>>().map_err(logged(
        "reading",
        "translation_keys",
        DatabaseError::NextRowFailure,
    ))
}

/// Adds a key and answers its id.
pub fn insert_key(
    conn: &Connection,
    namespace: &str,
    name: &str,
    audience: Audience,
    hint: Option<&str>,
) -> Result<i64, DatabaseError> {
    conn.execute(
        "INSERT INTO translation_keys (namespace, name, audience, hint) VALUES (?1, ?2, ?3, ?4)",
        params![namespace, name, audience, hint],
    )
    .map_err(logged(
        "writing",
        "translation_keys",
        DatabaseError::InsertionFailure,
    ))?;
    Ok(conn.last_insert_rowid())
}

pub fn update_key(
    conn: &Connection,
    id: i64,
    audience: Audience,
    hint: Option<&str>,
) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE translation_keys SET audience = ?1, hint = ?2 WHERE id = ?3",
        params![audience, hint, id],
    )
    .map_err(logged(
        "writing",
        "translation_keys",
        DatabaseError::UpdatingFailure,
    ))?;
    Ok(())
}

/// Deletes a key; the foreign key's cascade takes its texts with it.
pub fn delete_key(conn: &Connection, id: i64) -> Result<(), DatabaseError> {
    conn.execute("DELETE FROM translation_keys WHERE id = ?1", [id])
        .map_err(logged(
            "deleting from",
            "translation_keys",
            DatabaseError::ConnectionExecutableFailure,
        ))?;
    Ok(())
}

/// A language's texts, by key id.
pub fn texts_of(conn: &Connection, code: &str) -> Result<BTreeMap<i64, String>, DatabaseError> {
    let mut stmt = conn
        .prepare("SELECT key_id, value FROM translations WHERE locale_code = ?1")
        .map_err(logged(
            "reading",
            "translations",
            DatabaseError::TaskPreparationFailure,
        ))?;
    let rows = stmt
        .query_map([code], |row| Ok((row.get("key_id")?, row.get("value")?)))
        .map_err(logged(
            "reading",
            "translations",
            DatabaseError::QueryFailure,
        ))?;
    rows.collect::<Result<_, _>>().map_err(logged(
        "reading",
        "translations",
        DatabaseError::NextRowFailure,
    ))
}

/// Stores a language's text for a key, replacing the one it had.
pub fn save_text(
    conn: &Connection,
    code: &str,
    key_id: i64,
    value: &str,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO translations (locale_code, key_id, value) VALUES (?1, ?2, ?3) \
         ON CONFLICT (locale_code, key_id) DO UPDATE SET value = excluded.value",
        params![code, key_id, value],
    )
    .map_err(logged(
        "writing",
        "translations",
        DatabaseError::InsertionFailure,
    ))?;
    Ok(())
}

pub fn delete_text(conn: &Connection, code: &str, key_id: i64) -> Result<(), DatabaseError> {
    conn.execute(
        "DELETE FROM translations WHERE locale_code = ?1 AND key_id = ?2",
        params![code, key_id],
    )
    .map_err(logged(
        "deleting from",
        "translations",
        DatabaseError::ConnectionExecutableFailure,
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::migrated_connection;

    #[test]
    fn an_audience_is_stored_as_its_name_and_read_back() {
        let conn = migrated_connection();
        for audience in [Audience::Frontend, Audience::Backend] {
            let stored: String = conn
                .query_row("SELECT ?1", [audience], |row| row.get(0))
                .unwrap();
            assert_eq!(stored, audience.as_str());
            let read: Audience = conn
                .query_row("SELECT ?1", [audience.as_str()], |row| row.get(0))
                .unwrap();
            assert_eq!(read, audience);
        }
    }

    #[test]
    fn an_unknown_audience_is_an_error() {
        let conn = migrated_connection();
        let read = conn.query_row("SELECT 'admin'", [], |row| row.get::<_, Audience>(0));
        assert!(read.is_err(), "got {read:?}");
    }

    fn default_language(conn: &Connection) -> Option<String> {
        conn.query_row("SELECT code FROM locales WHERE is_default", [], |row| {
            row.get(0)
        })
        .ok()
    }

    #[test]
    fn a_language_becomes_the_default_only_while_none_is_and_only_when_enabled() {
        let conn = migrated_connection();
        insert_language_if_new(&conn, "qaa", "Disabled", None, 1).unwrap();
        conn.execute("UPDATE locales SET enabled = FALSE WHERE code = 'qaa'", [])
            .unwrap();
        make_default_if_none(&conn, "qaa").unwrap();
        assert_eq!(default_language(&conn), None, "a disabled language");

        insert_language_if_new(&conn, "qab", "First", None, 2).unwrap();
        insert_language_if_new(&conn, "qac", "Second", None, 3).unwrap();
        make_default_if_none(&conn, "qab").unwrap();
        make_default_if_none(&conn, "qac").unwrap();
        assert_eq!(default_language(&conn).as_deref(), Some("qab"));
    }

    #[test]
    fn an_existing_language_keeps_its_row() {
        let conn = migrated_connection();
        assert!(insert_language_if_new(&conn, "qaa", "Mine", Some("📕"), 1).unwrap());
        assert!(!insert_language_if_new(&conn, "qaa", "Shipped", None, 9).unwrap());

        let row: (String, Option<String>, i64) = conn
            .query_row(
                "SELECT display_name, icon, sort_order FROM locales WHERE code = 'qaa'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, ("Mine".to_string(), Some("📕".to_string()), 1));
    }
}
