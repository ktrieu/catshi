ALTER TABLE markets ADD COLUMN tag TEXT;

CREATE INDEX markets_tag_idx ON markets(tag);
