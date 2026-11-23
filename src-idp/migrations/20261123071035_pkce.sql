ALTER TABLE applications_oidc
ADD COLUMN pkce boolean not null
DEFAULT true;
