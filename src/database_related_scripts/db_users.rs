//! User queries the generic traits can't express, and how a `Role` is stored.

use rusqlite::{
    types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
    Connection, ToSql,
};

use crate::{error_types::DatabaseError, models::Role};

impl ToSql for Role {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for Role {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "admin" => Ok(Role::Admin),
            "member" => Ok(Role::Member),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

/// Whether any account exists yet.
pub fn any_exists(conn: &Connection) -> Result<bool, DatabaseError> {
    conn.query_row("SELECT EXISTS (SELECT 1 FROM users)", [], |row| row.get(0))
        .map_err(|err| {
            tracing::error!("checking whether any account exists failed: {err}");
            DatabaseError::QueryFailure
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_user, memory_connection};

    #[test]
    fn any_exists_is_false_only_for_an_empty_table() {
        let conn = memory_connection();
        assert!(!any_exists(&conn).unwrap());

        insert_user(&conn, "alice");
        assert!(any_exists(&conn).unwrap());
    }

    #[test]
    fn a_role_is_stored_as_its_name_and_read_back() {
        let conn = memory_connection();
        for role in [Role::Admin, Role::Member] {
            let stored: String = conn
                .query_row("SELECT ?1", [role], |row| row.get(0))
                .unwrap();
            assert_eq!(stored, role.as_str());
            let read: Role = conn
                .query_row("SELECT ?1", [role.as_str()], |row| row.get(0))
                .unwrap();
            assert_eq!(read, role);
        }
    }

    #[test]
    fn an_unknown_role_is_an_error_not_a_member() {
        let conn = memory_connection();
        let read = conn.query_row("SELECT 'owner'", [], |row| row.get::<_, Role>(0));
        assert!(read.is_err(), "got {read:?}");
    }
}
