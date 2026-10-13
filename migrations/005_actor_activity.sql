-- Legacy actors were only created by rotation, so they have observed events.
-- Identity placeholders introduced with browser linking must opt out explicitly.
ALTER TABLE clickman_actors ADD COLUMN has_events boolean NOT NULL DEFAULT true;
