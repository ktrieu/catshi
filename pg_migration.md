# SQLite → Postgres store migration principles

Pattern extracted from commits `38f115d`..`9d3e64c`, which introduced the
`UserStore` trait and ported callers off the old free functions in
`common/src/store/user.rs`.

**1. Split "how to reach the DB" from "what a store does."**
`DbExecutor` (`common/src/store.rs`) is a trait with `sqlite()`/`psql()`
accessors, implemented by both `CatshiConn` (pooled connections) and
`CatshiTx` (a transaction pair). Store trait methods take
`&mut impl DbExecutor` instead of a concrete `SqliteConnection`, so the same
method works whether the caller has a live connection or an open
transaction.

**2. The store itself is a trait + zero-sized impl, not free functions.**
`UserStore` (`common/src/store/user.rs`) declares the operations
(`get_by_id`, `get_by_discord_id`, `create_if_not_exists`, …); `DbUserStore
{}` implements it. `#[make(Send)]` (trait_variant) keeps the trait
dyn/generic-friendly with async methods. This is the seam for eventually
swapping in a mock in tests — the old code was just top-level `pub async fn`
in the module.

**3. Every migrated method dual-writes/dual-reads during the sqlite→postgres
migration, non-fatally.**
Each `DbUserStore` method runs the real query against sqlite (still the
source of truth — errors from it propagate), then fires the equivalent
query at postgres and only *logs* on mismatch/failure via
`log_pg_write_err` / `log_pg_compare_result` (`common/src/store.rs`). This
is shadow-mode validation before cutover, not a hard dependency on pg.

**4. Migrate one function at a time, in three separate commits.**
For each free function:
- (a) add its trait method to `UserStore`/`DbUserStore` alongside the old
  free function — nothing calls it yet
- (b) a dedicated commit ports every call site from `store::user::foo(...)`
  to `handler.user_store.foo(&mut tx, ...)`
- (c) once nothing references the free function, a final commit deletes it

See `eee89a0`/`97daedf` for `get_system_user`, `0ef1925`/`3e9a6db` for
`get_by_id`, `9d3e64c` for `get_by_discord_id`. Small, easily-revertable,
always-compiling commits at each step.

**5. Un-migrated helpers stay on raw sqlite, and call sites reach in
explicitly.**
Functions like `transfer::persist_transfer`, `blackjack::*`, `position::*`
haven't been ported yet, so they still take a plain sqlite executor.
Callers that already hold a `CatshiTx` pass `tx.sqlite_tx()` down into them
(`bot/src/command/resolve.rs`, `trade.rs`). This makes the migration
boundary visible in the diff instead of forcing a big-bang rewrite of every
store module at once.

**6. The store instance is dependency-injected, not looked up globally.**
`Handler` gets a `db: CatshiDb` and `user_store: DbUserStore` field,
constructed once in `main()` and threaded through everywhere
(`bot/src/main.rs`). Call sites use `handler.user_store.get_by_id(...)`
rather than importing a module function directly — that's what makes step
4's swap mechanical.

**7. Cast narrow postgres integer/timestamp columns before decoding into
`i64` fields.**
Postgres `id` columns (and other integer columns like `sender`/`receiver`/
`user_id`) are declared `INT GENERATED ALWAYS AS IDENTITY` or plain
`INTEGER` — 4 bytes — while sqlite's `INTEGER` is always 8 bytes and every
store struct field is typed `i64`. sqlx's postgres decoder does not
implicitly widen `INT4` into `i64`, so any hand-written `query_as(...)` /
`query_as::<_, T>(...)` against postgres (the sqlite-side `query_as!` macro
is compile-time checked against the sqlite schema, so it isn't affected)
that reads one of these columns straight into an `i64` field fails to
decode with a type mismatch. Cast explicitly wherever this happens —
`CAST(id AS BIGINT) as id`, `CAST(users.id AS BIGINT) as id` in joins,
etc. — as done throughout `user.rs`, `transfer.rs`, and `blackjack.rs`.
`TIMESTAMPTZ` columns read into `i64` need the same treatment:
`EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at`.

Because these postgres reads/writes only run in shadow mode (principle 3)
a missing cast won't fail the build or crash the bot — it just logs a
decode-mismatch/failure warning on every call. Check the logs after
porting a new store's postgres queries to catch these.

The throughline: keep sqlite authoritative and never let the new path break
behavior, migrate surface area in the smallest possible increments, and let
the trait boundary double as both the pg-shadow seam and a future
test-mock seam.
