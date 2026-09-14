-- These columns store Currency values (bips) or share quantities but were
-- declared INTEGER, capping them at ~2.1 billion bips (~2,147 YP) before
-- silently erroring. users.cash_balance is already BIGINT for the same
-- reason. Widening INTEGER -> BIGINT is a lossless conversion for every
-- existing value.
ALTER TABLE blackjacks ALTER COLUMN staked TYPE BIGINT;

ALTER TABLE orders ALTER COLUMN quantity TYPE BIGINT;
ALTER TABLE orders ALTER COLUMN shares_price TYPE BIGINT;
ALTER TABLE orders ALTER COLUMN fees TYPE BIGINT;
ALTER TABLE orders ALTER COLUMN cost_basis TYPE BIGINT;

ALTER TABLE positions ALTER COLUMN quantity TYPE BIGINT;
ALTER TABLE positions ALTER COLUMN cost_basis TYPE BIGINT;

ALTER TABLE tips ALTER COLUMN amount TYPE BIGINT;

ALTER TABLE transfers ALTER COLUMN amount TYPE BIGINT;
