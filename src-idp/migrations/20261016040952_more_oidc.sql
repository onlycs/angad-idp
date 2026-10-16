-- Add migration script here
ALTER TABLE applications_oidc
ALTER COLUMN client_secret
DROP not null;

ALTER TABLE applications_oidc
ADD COLUMN match_url text not null;
