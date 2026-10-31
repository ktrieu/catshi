DROP INDEX markets_tag_idx;

CREATE UNIQUE INDEX markets_tag_idx ON markets(tag);
