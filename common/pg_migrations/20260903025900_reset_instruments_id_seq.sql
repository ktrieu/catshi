-- Part of the instruments-store cutover to Postgres. `instruments` rows used to be
-- written to Postgres with `OVERRIDING SYSTEM VALUE` (ids copied from the
-- authoritative sqlite row), so the `GENERATED ALWAYS AS IDENTITY` sequence behind
-- `instruments.id` never advanced past its start value. Now that Postgres generates
-- the id itself, fast-forward the sequence past the highest existing id so the next
-- insert doesn't collide with an already-mirrored row. `false` => the given value is
-- handed out by the next `nextval`, so an empty table starts at 1.
SELECT setval(
    pg_get_serial_sequence('instruments', 'id'),
    (SELECT COALESCE(MAX(id), 0) + 1 FROM instruments),
    false
);
