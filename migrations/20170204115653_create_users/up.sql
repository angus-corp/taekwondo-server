CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,

    first_name TEXT  NOT NULL CHECK (first_name != ''),
    last_name  TEXT  NOT NULL CHECK (last_name != ''),
    username   TEXT  NOT NULL UNIQUE CHECK (length(username) <= 32 AND username ~ '^[a-z0-9](-?[a-z0-9])*$'),
    email      TEXT  NOT NULL UNIQUE CHECK (email ~ '^.+@.+$'),
    password   BYTEA NOT NULL
);
