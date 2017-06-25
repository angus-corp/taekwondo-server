ALTER TABLE users ALTER COLUMN first_name TYPE TEXT;
ALTER TABLE users ALTER COLUMN last_name TYPE TEXT;
ALTER TABLE users ALTER COLUMN username TYPE TEXT;
ALTER TABLE users ALTER COLUMN email TYPE TEXT;

DROP INDEX users_email_key;
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);
DROP INDEX users_username_key;
ALTER TABLE users ADD CONSTRAINT users_username_key UNIQUE (username);
