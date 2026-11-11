//! Account management by an admin: listing the users, changing a user's role, and marking an
//! email address verified.

use std::sync::Arc;

use rusqlite::Connection;

use crate::{
    data_models::users_model::{UserChanges, UserSummary},
    db::{convert_system_time_to_unix_time, Database},
    error_types::{AppErrors, AuthenticationError, DatabaseError},
    models::{QuerySeparator, Role, UserDatabaseColumns, UserMetadata, WithId},
};

/// Every account, in id order.
pub async fn all_users(db: Arc<Database>) -> Result<Vec<WithId<UserSummary>>, AppErrors> {
    let users = db
        .read(|conn| {
            Ok(Database::get_all_entries_with_connection::<UserMetadata>(
                conn,
            )?)
        })
        .await?;
    Ok(users.into_iter().map(|row| summary(row.data)).collect())
}

/// Applies an admin's changes to one account and answers it as it is now. Only marking an email
/// verified is allowed, not the reverse, and the last admin can't be made a member.
pub async fn change_user(
    db: Arc<Database>,
    user_id: i64,
    changes: UserChanges,
) -> Result<WithId<UserSummary>, AppErrors> {
    let UserChanges {
        role,
        email_verified,
    } = changes;
    let nothing_to_change = role.is_none() && email_verified.is_none();
    if nothing_to_change || email_verified == Some(false) {
        return Err(AppErrors::InvalidRequest);
    }

    db.transact(move |conn| {
        let user = Database::get_entry_with_connection::<UserMetadata>(conn, user_id)?;
        if let Some(role) = role {
            // Counted inside the write lock: of two demotions at once, the second sees the first.
            if user.role == Role::Admin && role != Role::Admin && admin_count(conn)? == 1 {
                return Err(AuthenticationError::LastAdmin.into());
            }
            Database::update_value_with_connection::<UserMetadata>(
                conn,
                UserDatabaseColumns::Role,
                &role,
                user_id,
            )?;
        }
        if email_verified == Some(true) {
            Database::update_value_with_connection::<UserMetadata>(
                conn,
                UserDatabaseColumns::EmailVerified,
                &true,
                user_id,
            )?;
        }
        let user = Database::get_entry_with_connection::<UserMetadata>(conn, user_id)?;
        Ok(summary(user))
    })
    .await
}

fn admin_count(conn: &Connection) -> Result<usize, DatabaseError> {
    let admins = Database::search_for_multiple_rows_with_connection::<UserMetadata>(
        conn,
        &[UserDatabaseColumns::Role],
        &[&Role::Admin],
        QuerySeparator::And,
    )?;
    Ok(admins.len())
}

/// The account without its password hash.
fn summary(user: UserMetadata) -> WithId<UserSummary> {
    WithId {
        id: user.user_id,
        data: UserSummary {
            username: user.username,
            email: user.email,
            email_verified: user.email_verified,
            role: user.role,
            created_at: convert_system_time_to_unix_time(user.created_at),
            last_login: user.last_login.map(convert_system_time_to_unix_time),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use rusqlite::params;

    use super::*;
    use crate::{
        data_models::authentication_model::{LoginRequest, LoginResponse},
        routes::{auth::hash_secret, login::log_in},
        test_support::{insert_user, TempDatabase},
    };

    fn add_user(temp: &TempDatabase, username: &str, role: &str) -> i64 {
        temp.db
            .setup_new_transaction(|conn| {
                let user = insert_user(conn, username);
                conn.execute(
                    "UPDATE users SET role = ?1 WHERE id = ?2",
                    params![role, user],
                )
                .unwrap();
                Ok(user)
            })
            .unwrap()
    }

    fn role_of(temp: &TempDatabase, user_id: i64) -> String {
        temp.db
            .setup_new_transaction(|conn| {
                Ok(conn
                    .query_row("SELECT role FROM users WHERE id = ?1", [user_id], |row| {
                        row.get(0)
                    })
                    .unwrap())
            })
            .unwrap()
    }

    fn changes(role: Option<Role>, email_verified: Option<bool>) -> UserChanges {
        UserChanges {
            role,
            email_verified,
        }
    }

    async fn change(
        temp: &TempDatabase,
        user_id: i64,
        role: Option<Role>,
        email_verified: Option<bool>,
    ) -> Result<WithId<UserSummary>, AppErrors> {
        change_user(Arc::clone(&temp.db), user_id, changes(role, email_verified)).await
    }

    fn is_last_admin<T>(result: &Result<T, AppErrors>) -> bool {
        matches!(
            result,
            Err(AppErrors::Authentication(AuthenticationError::LastAdmin))
        )
    }

    #[tokio::test]
    async fn the_list_has_every_user_in_id_order() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice", "admin");
        let bob = add_user(&temp, "bob", "member");

        let users = all_users(Arc::clone(&temp.db)).await.unwrap();

        let listed: Vec<(i64, &str, &str, Role, bool)> = users
            .iter()
            .map(|user| {
                (
                    user.id,
                    user.data.username.as_str(),
                    user.data.email.as_str(),
                    user.data.role,
                    user.data.email_verified,
                )
            })
            .collect();
        assert_eq!(
            listed,
            vec![
                (alice, "alice", "alice@example.com", Role::Admin, false),
                (bob, "bob", "bob@example.com", Role::Member, false),
            ]
        );
        assert!(users.iter().all(|user| user.data.last_login.is_none()));
    }

    #[tokio::test]
    async fn a_member_is_made_admin_and_member_again() {
        let temp = TempDatabase::open();
        add_user(&temp, "alice", "admin");
        let bob = add_user(&temp, "bob", "member");

        let promoted = change(&temp, bob, Some(Role::Admin), None).await.unwrap();
        assert_eq!(promoted.id, bob);
        assert_eq!(promoted.data.role, Role::Admin);
        assert_eq!(role_of(&temp, bob), "admin");

        let demoted = change(&temp, bob, Some(Role::Member), None).await.unwrap();
        assert_eq!(demoted.data.role, Role::Member);
        assert_eq!(role_of(&temp, bob), "member");
    }

    #[tokio::test]
    async fn the_last_admin_stays_an_admin() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice", "admin");
        let bob = add_user(&temp, "bob", "admin");

        change(&temp, bob, Some(Role::Member), None).await.unwrap();

        let refused = change(&temp, alice, Some(Role::Member), None).await;
        assert!(is_last_admin(&refused));
        assert_eq!(role_of(&temp, alice), "admin");
        // Admin again is no demotion, so the last admin may be "changed" to admin.
        change(&temp, alice, Some(Role::Admin), None).await.unwrap();
    }

    #[tokio::test]
    async fn of_two_parallel_demotions_of_the_only_two_admins_exactly_one_wins() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice", "admin");
        let bob = add_user(&temp, "bob", "admin");

        let (a, b) = tokio::join!(
            change(&temp, alice, Some(Role::Member), None),
            change(&temp, bob, Some(Role::Member), None),
        );

        assert_eq!([a.is_ok(), b.is_ok()].iter().filter(|ok| **ok).count(), 1);
        assert!(is_last_admin(&a) || is_last_admin(&b));
        let admins = [role_of(&temp, alice), role_of(&temp, bob)]
            .iter()
            .filter(|role| *role == "admin")
            .count();
        assert_eq!(admins, 1);
    }

    #[tokio::test]
    async fn an_email_marked_verified_lets_the_user_log_in() {
        let temp = TempDatabase::open();
        add_user(&temp, "alice", "admin");
        let bob = add_user(&temp, "bob", "member");
        let hash = hash_secret("correct horse battery".to_string())
            .await
            .unwrap();
        temp.db
            .setup_new_transaction(|conn| {
                conn.execute(
                    "UPDATE users SET password_hash = ?1 WHERE id = ?2",
                    params![hash, bob],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
        let login = || {
            let request = LoginRequest {
                username_or_email: "bob".to_string(),
                password: "correct horse battery".to_string(),
                device_name: "phone".to_string(),
                device_id: None,
                platform: "ios".to_string(),
            };
            log_in(Arc::clone(&temp.db), request, SystemTime::now())
        };
        assert!(matches!(
            login().await,
            Err(AppErrors::Authentication(
                AuthenticationError::EmailNotVerified
            ))
        ));

        let verified = change(&temp, bob, None, Some(true)).await.unwrap();

        assert!(verified.data.email_verified);
        assert!(matches!(login().await, Ok(LoginResponse::Ok(_))));
    }

    #[tokio::test]
    async fn no_change_or_an_unverified_flag_is_refused_before_anything_is_written() {
        let temp = TempDatabase::open();
        add_user(&temp, "alice", "admin");
        let bob = add_user(&temp, "bob", "member");

        for (role, email_verified) in [
            (None, None),
            (None, Some(false)),
            (Some(Role::Admin), Some(false)),
        ] {
            let result = change(&temp, bob, role, email_verified).await;
            assert!(
                matches!(result, Err(AppErrors::InvalidRequest)),
                "{role:?} {email_verified:?}"
            );
        }
        assert_eq!(role_of(&temp, bob), "member", "the role stays as it was");
    }

    #[tokio::test]
    async fn an_unknown_user_is_not_found() {
        let temp = TempDatabase::open();
        let alice = add_user(&temp, "alice", "admin");

        let result = change(&temp, alice + 1, Some(Role::Admin), None).await;

        assert!(matches!(
            result,
            Err(AppErrors::Database(DatabaseError::EntryNotFound))
        ));
    }
}
