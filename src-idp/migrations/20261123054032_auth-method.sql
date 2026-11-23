CREATE TYPE tokenauthmethod AS ENUM ('client_secret_basic', 'client_secret_post');

ALTER TABLE applications_oidc
ADD COLUMN token_endpoint_auth_method tokenauthmethod not null
DEFAULT 'client_secret_basic';
