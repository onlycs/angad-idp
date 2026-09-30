CREATE TABLE users (
    -- identity
    id          text not null   PRIMARY KEY,
    username    text not null   UNIQUE,
    email       text not null   UNIQUE,

    -- authentication
    password_hash   text not null,

    -- timestamps
    created_at  timestamptz not null    DEFAULT now()
);

CREATE TABLE applications (
    slug    text not null   PRIMARY KEY,
    name    text not null   UNIQUE,
    url     text not null,

    created_at  timestamptz not null    DEFAULT now()
);

CREATE TABLE applications_oidc (
    -- identity
    slug  text not null   PRIMARY KEY REFERENCES applications(slug) ON DELETE CASCADE,

    -- authelia generation
    client_secret   text not null,
    redirect_uris   text[] not null,
);

CREATE TABLE roles (
    uid             text not null       REFERENCES users(id) ON DELETE CASCADE,
    application     text not null       REFERENCES applications(slug) ON DELETE CASCADE,
    admin           boolean not null    DEFAULT false,

    PRIMARY KEY (uid, application)
);

CREATE TABLE integrations (
    key_sha     bytea not null      PRIMARY KEY,
    name        text not null       UNIQUE,
    read_only   boolean not null,

    created_at  timestamptz not null    DEFAULT now()
);

CREATE TABLE invites (
    key_sha     bytea not null  PRIMARY KEY,
    email       text not null   UNIQUE,

    created_at  timestamptz not null    DEFAULT now()
);

CREATE TABLE invite_roles (
    invite          bytea not null      REFERENCES invites(key_sha) ON DELETE CASCADE,
    application     text not null       REFERENCES applications(slug) ON DELETE CASCADE,
    admin           boolean not null    DEFAULT false,

    PRIMARY KEY (invite, application)
);

INSERT INTO applications (slug, name)
VALUES ("domain", "Domain");
