use sqlx::{Executor, Sqlite, Transaction, query, query_as};
use trait_variant::make;

use crate::{
    currency::Currency,
    store::{
        CatshiTx, DbExecutor, log_pg_compare_result,
        user::{self, DbUser, UserStore},
    },
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum TransferSource {
    Unknown,
    Deposit,
    UserInitiated,
    Order,
    TradeFee,
    Gambling,
    MessageTip,
}

#[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Transfer {
    pub id: i64,
    pub amount: Currency,
    pub sender: i64,
    pub receiver: i64,
    pub memo: String,
    pub created_at: i64,
    pub source: TransferSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTransfer {
    pub amount: Currency,
    pub sender: i64,
    pub receiver: i64,
    pub memo: String,
    pub source: TransferSource,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UserTransfersBySource {
    pub user: DbUser,
    pub source: TransferSource,
    pub net: Currency,
}

// TODO: delete once callers are ported to `TransferStore::persist`.
//
// Save a transaction row and make the necessary balance updates.
// Because it makes three queries that must occur together it takes a transaction and not a generic Connection.
pub async fn persist_transfer(
    tx: &mut Transaction<'_, Sqlite>,
    create: &CreateTransfer,
) -> anyhow::Result<Transfer> {
    let transfer = query_as!(
        Transfer,
        r#"
        INSERT INTO transfers (
            amount,
            sender,
            receiver,
            memo,
            source
        ) VALUES ($1, $2, $3, $4, $5)
        RETURNING
            id,
            amount as "amount: Currency",
            sender,
            receiver,
            memo,
            created_at,
            source as "source: TransferSource"
        "#,
        create.amount,
        create.sender,
        create.receiver,
        create.memo,
        create.source,
    )
    .fetch_one(&mut **tx)
    .await?;

    // Credit the receiving account.
    user::increment_balance_by_user_id(&mut **tx, create.receiver, create.amount).await?;

    // Debit the sending account.
    user::increment_balance_by_user_id(&mut **tx, create.sender, -create.amount).await?;

    Ok(transfer)
}

// TODO: delete once callers are ported to `TransferStore::get_net_user_transfers_by_source`.
pub async fn get_net_user_transfers_by_source(
    exec: impl Executor<'_, Database = Sqlite>,
) -> anyhow::Result<Vec<UserTransfersBySource>> {
    let rows = query!(
        r#"
        SELECT
            users.id,
            users.name,
            users.discord_id,
            users.cash_balance as "cash_balance: Currency",
            source as "source: TransferSource",
            SUM(net_amount) AS "net: Currency"
        FROM (
            SELECT
                receiver AS user_id,
                source,
                amount AS net_amount
            FROM transfers
            UNION ALL
            SELECT
                sender AS user_id,
                source,
                -amount AS net_amount
            FROM transfers
        ) t
        JOIN users ON users.id = user_id
        GROUP BY user_id, source
        "#,
    )
    .fetch_all(exec)
    .await?;

    let sums = rows
        .into_iter()
        .map(|r| UserTransfersBySource {
            user: DbUser {
                id: r.id,
                discord_id: r.discord_id,
                name: r.name,
                cash_balance: r.cash_balance,
            },
            source: r.source,
            net: r.net,
        })
        .collect();

    Ok(sums)
}

#[make(Send)]
pub trait TransferStore {
    // Because it makes three queries that must occur together it takes a transaction
    // and not a generic connection.
    async fn persist(
        &self,
        tx: &mut CatshiTx,
        user_store: &(impl UserStore + Sync),
        create: &CreateTransfer,
    ) -> anyhow::Result<Transfer>;

    async fn get_net_user_transfers_by_source(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<UserTransfersBySource>>;
}

pub struct DbTransferStore {}

impl TransferStore for DbTransferStore {
    async fn persist(
        &self,
        tx: &mut CatshiTx,
        user_store: &(impl UserStore + Sync),
        create: &CreateTransfer,
    ) -> anyhow::Result<Transfer> {
        let transfer = query_as!(
            Transfer,
            r#"
            INSERT INTO transfers (
                amount,
                sender,
                receiver,
                memo,
                source
            ) VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id,
                amount as "amount: Currency",
                sender,
                receiver,
                memo,
                created_at,
                source as "source: TransferSource"
            "#,
            create.amount,
            create.sender,
            create.receiver,
            create.memo,
            create.source,
        )
        .fetch_one(tx.sqlite())
        .await?;

        // Credit the receiving account.
        user_store
            .increment_balance_by_id(tx, create.receiver, create.amount)
            .await?;

        // Debit the sending account.
        user_store
            .increment_balance_by_id(tx, create.sender, -create.amount)
            .await?;

        // Mirror the insert to postgres. `id` is forced to match the sqlite row since
        // `tips` references `transfers(id)` by foreign key. `created_at` is cast back
        // to a unix-epoch bigint (postgres stores it as TIMESTAMPTZ, defaulted
        // independently by `CURRENT_TIMESTAMP` rather than passed in) so it decodes
        // into the same `Transfer` shape as the sqlite row for comparison; the two
        // timestamps can occasionally differ by a second since they're set by separate
        // statements, not copied from one write to the other.
        let pg_result: sqlx::Result<Transfer> = query_as(
            r#"
            INSERT INTO transfers (
                id,
                amount,
                sender,
                receiver,
                memo,
                source
            )
            OVERRIDING SYSTEM VALUE
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id,
                amount,
                sender,
                receiver,
                memo,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at,
                source
            "#,
        )
        .bind(transfer.id)
        .bind(create.amount)
        .bind(create.sender)
        .bind(create.receiver)
        .bind(&create.memo)
        .bind(create.source)
        .fetch_one(tx.psql())
        .await;

        log_pg_compare_result(pg_result, &transfer, "transfer persist");

        Ok(transfer)
    }

    async fn get_net_user_transfers_by_source(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<UserTransfersBySource>> {
        let rows = query!(
            r#"
            SELECT
                users.id,
                users.name,
                users.discord_id,
                users.cash_balance as "cash_balance: Currency",
                source as "source: TransferSource",
                SUM(net_amount) AS "net: Currency"
            FROM (
                SELECT
                    receiver AS user_id,
                    source,
                    amount AS net_amount
                FROM transfers
                UNION ALL
                SELECT
                    sender AS user_id,
                    source,
                    -amount AS net_amount
                FROM transfers
            ) t
            JOIN users ON users.id = user_id
            GROUP BY user_id, source
            ORDER BY user_id, source
            "#,
        )
        .fetch_all(db.sqlite())
        .await?;

        let sums: Vec<UserTransfersBySource> = rows
            .into_iter()
            .map(|r| UserTransfersBySource {
                user: DbUser {
                    id: r.id,
                    discord_id: r.discord_id,
                    name: r.name,
                    cash_balance: r.cash_balance,
                },
                source: r.source,
                net: r.net,
            })
            .collect();

        let pg_result: sqlx::Result<Vec<UserTransfersBySource>> =
            query_as::<_, PgUserTransfersBySourceRow>(
                r#"
                SELECT
                    CAST(users.id AS BIGINT) as id,
                    users.name,
                    users.discord_id,
                    users.cash_balance,
                    source,
                    SUM(net_amount) AS net
                FROM (
                    SELECT
                        receiver AS user_id,
                        source,
                        amount AS net_amount
                    FROM transfers
                    UNION ALL
                    SELECT
                        sender AS user_id,
                        source,
                        -amount AS net_amount
                    FROM transfers
                ) t
                JOIN users ON users.id = t.user_id
                GROUP BY users.id, source
                ORDER BY users.id, source
                "#,
            )
            .fetch_all(db.psql())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|r| UserTransfersBySource {
                        user: DbUser {
                            id: r.id,
                            discord_id: r.discord_id,
                            name: r.name,
                            cash_balance: r.cash_balance,
                        },
                        source: r.source,
                        net: r.net,
                    })
                    .collect()
            });

        log_pg_compare_result(pg_result, &sums, "get_net_user_transfers_by_source");

        Ok(sums)
    }
}

#[derive(sqlx::FromRow)]
struct PgUserTransfersBySourceRow {
    id: i64,
    name: String,
    discord_id: String,
    cash_balance: Currency,
    source: TransferSource,
    net: Currency,
}
