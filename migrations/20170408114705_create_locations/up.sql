CREATE TABLE locations (
    id BIGSERIAL PRIMARY KEY,

    name    TEXT             NOT NULL CHECK (name != ''),
    address TEXT             NOT NULL CHECK (address != ''),
    lat     DOUBLE PRECISION NOT NULL,
    lng     DOUBLE PRECISION NOT NULL
);

CREATE TABLE instructor_locations (
    id            BIGSERIAL PRIMARY KEY, -- Unnecessary, but many frameworks demand it.
    instructor_id BIGINT    NOT NULL REFERENCES users ON UPDATE CASCADE ON DELETE CASCADE,
    location_id   BIGINT    NOT NULL REFERENCES locations ON UPDATE CASCADE ON DELETE CASCADE
);

ALTER TABLE users ADD COLUMN training_location BIGINT REFERENCES locations ON UPDATE CASCADE ON DELETE SET NULL;
