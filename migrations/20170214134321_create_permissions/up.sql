CREATE TABLE roles (
    id BIGSERIAL PRIMARY KEY,

    privileges  BIGINT[] NOT NULL,
    name        TEXT     NOT NULL CHECK (name != ''),
    description TEXT     NOT NULL
);

CREATE TABLE user_roles (
    id      BIGSERIAL PRIMARY KEY, -- Unnecessary, but many frameworks demand it.
    user_id BIGINT    NOT NULL REFERENCES users ON UPDATE CASCADE ON DELETE CASCADE,
    role_id BIGINT    NOT NULL REFERENCES roles ON UPDATE CASCADE ON DELETE CASCADE
);
