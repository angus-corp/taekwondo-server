ALTER TABLE users DROP CONSTRAINT users_email_key;
CREATE UNIQUE INDEX users_email_key ON users (lower(email));
ALTER TABLE users DROP CONSTRAINT users_username_key;
CREATE UNIQUE INDEX users_username_key ON users (lower(username));
