use std::env;

use sqlx::PgPool;

use crate::invite;

#[derive(sqlx::FromRow)]
pub struct Response {
    username: String,
    email: String,
    password_hash: Option<String>,
}

async fn parse(pg: &PgPool) -> Result<Vec<Response>, sqlx::Error> {
    sqlx::query_as::<_, Response>(
        r#"
        SELECT
            u.username,
            h.email,
            e.payload->>'encodedHash' AS password_hash
        FROM projections.users14 u
        LEFT JOIN projections.users14_humans h
            ON h.user_id = u.id
            AND h.instance_id = u.instance_id
        LEFT JOIN auth.users3 a
            ON a.id = u.id
            AND a.instance_id = u.instance_id
        LEFT JOIN LATERAL (
            SELECT payload
            FROM eventstore.events2
            WHERE aggregate_id = u.id
              AND instance_id = u.instance_id
              AND aggregate_type = 'user'
              AND payload ? 'encodedHash'
            ORDER BY sequence DESC
            LIMIT 1
        ) e ON a.password_set IS TRUE
        WHERE u.type = 1
        ORDER BY u.username;
        "#,
    )
    .fetch_all(pg)
    .await
}

async fn import(pg: &PgPool, users: Vec<Response>) -> Result<(), sqlx::Error> {
    let mut tx = pg.begin().await?;

    for user in users {
        let Some(hash) = user.password_hash else {
            invite::create_impl(
                user.email,
                vec![],
                chrono::Utc::now() + chrono::Duration::days(14),
                pg,
            )
            .await
            .unwrap();

            continue;
        };

        let id = cuid2::create_id();

        sqlx::query!(
            r#"
            INSERT INTO users (
                id, username, email, password_hash, hasher
            )
            VALUES ($1, $2, $3, $4, 'bcrypt'::hashkind)
            "#,
            id,
            user.username,
            user.email,
            hash,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub(crate) async fn main() {
    let apool = sqlx::PgPool::connect(env::var("DATABASE_URL").unwrap().as_str())
        .await
        .unwrap();

    let zpool = sqlx::PgPool::connect(env::var("ZITADEL_DATABASE_URL").unwrap().as_str())
        .await
        .unwrap();

    let users = parse(&zpool).await.unwrap();
    import(&apool, users).await.unwrap();
}
