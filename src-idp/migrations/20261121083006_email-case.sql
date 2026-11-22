-- Add migration script here
CREATE UNIQUE INDEX users_email_lower_key ON users (lower(email));
