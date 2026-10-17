-- Add migration script here
ALTER TABLE applications_oidc
DROP COLUMN match_url;
