ALTER TABLE instructor_locations ADD CONSTRAINT
    duplicate_assignments UNIQUE (instructor_id, location_id);
