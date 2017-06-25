DROP INDEX users_email_key;
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);
DROP INDEX users_username_key;
ALTER TABLE users ADD CONSTRAINT users_username_key UNIQUE (username);
