CREATE EXTENSION IF NOT EXISTS citext;

ALTER TABLE users ALTER COLUMN first_name TYPE CITEXT;
ALTER TABLE users ALTER COLUMN last_name TYPE CITEXT;
ALTER TABLE users ALTER COLUMN username TYPE CITEXT;
ALTER TABLE users ALTER COLUMN email TYPE CITEXT;

ALTER TABLE users DROP CONSTRAINT users_email_key;
CREATE UNIQUE INDEX users_email_key ON users (lower(email));
ALTER TABLE users DROP CONSTRAINT users_username_key;
CREATE UNIQUE INDEX users_username_key ON users (lower(username));
