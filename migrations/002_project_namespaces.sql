ALTER TABLE clickman_projects ADD COLUMN namespace text NOT NULL DEFAULT '';
ALTER TABLE clickman_projects DROP CONSTRAINT clickman_projects_slug_key;
ALTER TABLE clickman_projects ADD CONSTRAINT clickman_projects_namespace_slug_key UNIQUE(namespace,slug);
