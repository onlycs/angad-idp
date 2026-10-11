-- Add migration script here
ALTER TABLE invites
ADD COLUMN expiry timestamptz not null;

ALTER TABLE invites
RENAME COLUMN key_sha TO key_hash;
