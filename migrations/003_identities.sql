ALTER TABLE clickman_actors ADD COLUMN merged_into_id bigint;
ALTER TABLE clickman_actors ADD CONSTRAINT clickman_actor_canonical_fk
 FOREIGN KEY(project_id,merged_into_id) REFERENCES clickman_actors(project_id,id);
ALTER TABLE clickman_actors ADD CONSTRAINT clickman_actor_not_self CHECK(merged_into_id IS NULL OR merged_into_id<>id);
CREATE INDEX clickman_actor_aliases ON clickman_actors(project_id,merged_into_id) WHERE merged_into_id IS NOT NULL;
