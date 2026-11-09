//! User queries the generic traits can't express.

use rusqlite::Connection;

use crate::error_types::DatabaseError;

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
}
