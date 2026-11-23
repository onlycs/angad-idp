CREATE TYPE hashkind AS ENUM ('bcrypt', 'argon2');

ALTER TABLE users
ADD COLUMN hasher hashkind not null
DEFAULT 'argon2';
