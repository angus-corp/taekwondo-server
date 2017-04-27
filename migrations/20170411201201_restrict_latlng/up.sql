ALTER TABLE locations ADD CONSTRAINT
    latlng_bounds CHECK (-90 <= lat AND lat <= 90 AND -180 <= lng AND lng < 180);
