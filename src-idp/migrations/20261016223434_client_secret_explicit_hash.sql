-- Add migration script here
ALTER TABLE applications_oidc
RENAME COLUMN client_secret TO client_secret_hash;
