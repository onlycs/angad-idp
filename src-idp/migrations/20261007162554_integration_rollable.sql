-- Add migration script here

ALTER TABLE integrations
ADD COLUMN id text not null;

ALTER TABLE integrations
DROP CONSTRAINT integrations_pkey;

ALTER TABLE integrations
ADD PRIMARY KEY (id);

ALTER TABLE integrations
RENAME COLUMN key_sha TO key_hash;
