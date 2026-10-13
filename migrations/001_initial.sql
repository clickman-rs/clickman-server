CREATE TABLE clickman_projects (
  id uuid PRIMARY KEY,
  slug text NOT NULL UNIQUE,
  name text NOT NULL,
  settings jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE clickman_write_keys (
  id uuid PRIMARY KEY,
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  source text NOT NULL,
  digest text NOT NULL UNIQUE,
  created_at timestamptz NOT NULL DEFAULT now(),
  revoked_at timestamptz
);
CREATE INDEX clickman_write_keys_project ON clickman_write_keys(project_id);
CREATE TABLE clickman_access_keys (
  id uuid PRIMARY KEY,
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  name text NOT NULL,
  role text NOT NULL CHECK (role IN ('reader', 'admin')),
  digest text NOT NULL UNIQUE,
  created_at timestamptz NOT NULL DEFAULT now(),
  revoked_at timestamptz
);
CREATE INDEX clickman_access_keys_project ON clickman_access_keys(project_id);
CREATE TABLE clickman_events (
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  message_id uuid NOT NULL,
  source text NOT NULL,
  occurred_at timestamptz NOT NULL,
  received_at timestamptz NOT NULL DEFAULT now(),
  external_id text NOT NULL,
  event text NOT NULL,
  properties jsonb NOT NULL,
  context jsonb NOT NULL,
  rotated_at timestamptz,
  PRIMARY KEY (project_id, message_id)
);
CREATE INDEX clickman_events_rotation ON clickman_events(project_id, received_at, message_id) WHERE rotated_at IS NULL;
CREATE INDEX clickman_events_actor ON clickman_events(project_id, external_id);
CREATE INDEX clickman_events_expiry ON clickman_events(project_id, received_at) WHERE rotated_at IS NOT NULL;
CREATE TABLE clickman_event_names (
  id bigserial PRIMARY KEY,
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  name text NOT NULL,
  UNIQUE(project_id, name),
  UNIQUE(project_id, id)
);
CREATE TABLE clickman_keys (
  id bigserial PRIMARY KEY,
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  key text NOT NULL,
  UNIQUE(project_id, key),
  UNIQUE(project_id, id)
);
CREATE TABLE clickman_actors (
  id bigserial PRIMARY KEY,
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  external_id text,
  first_seen_on date NOT NULL,
  erased_at timestamptz,
  UNIQUE(project_id, external_id),
  UNIQUE(project_id, id),
  CHECK ((external_id IS NULL) = (erased_at IS NOT NULL))
);
CREATE TABLE clickman_chunks (
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  day date NOT NULL,
  event_name_id bigint NOT NULL,
  seq integer NOT NULL,
  events integer NOT NULL CHECK (events BETWEEN 1 AND 4096),
  key_ids bigint[] NOT NULL,
  payload bytea NOT NULL,
  PRIMARY KEY (project_id, day, event_name_id, seq),
  FOREIGN KEY (project_id, event_name_id) REFERENCES clickman_event_names(project_id, id)
);
ALTER TABLE clickman_chunks ALTER COLUMN payload SET STORAGE EXTERNAL;
CREATE TABLE clickman_actor_days (
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  actor_id bigint NOT NULL,
  year integer NOT NULL,
  days bit varying(366) NOT NULL CHECK (bit_length(days) = 366),
  PRIMARY KEY (project_id, actor_id, year),
  FOREIGN KEY (project_id, actor_id) REFERENCES clickman_actors(project_id, id)
);
CREATE TABLE clickman_daily_counts (
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  day date NOT NULL,
  event_name_id bigint NOT NULL,
  events bigint NOT NULL,
  actors bigint NOT NULL,
  PRIMARY KEY (project_id, day, event_name_id),
  FOREIGN KEY (project_id, event_name_id) REFERENCES clickman_event_names(project_id, id)
);
CREATE TABLE clickman_funnels (
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  key text NOT NULL,
  definition jsonb NOT NULL,
  PRIMARY KEY (project_id, key)
);
CREATE TABLE clickman_reports (
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  key text NOT NULL,
  result jsonb NOT NULL,
  computed_at timestamptz NOT NULL,
  PRIMARY KEY (project_id, key)
);
CREATE TABLE clickman_destinations (
  id uuid PRIMARY KEY,
  project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
  name text NOT NULL,
  kind text NOT NULL,
  config jsonb NOT NULL,
  UNIQUE(project_id, name),
  UNIQUE(project_id, id)
);
CREATE TABLE clickman_deliveries (
  project_id uuid NOT NULL,
  destination_id uuid NOT NULL,
  message_id uuid NOT NULL,
  completed_at timestamptz,
  next_attempt_at timestamptz NOT NULL DEFAULT now(),
  attempts integer NOT NULL DEFAULT 0,
  last_error text,
  PRIMARY KEY (project_id, destination_id, message_id),
  FOREIGN KEY (project_id, destination_id) REFERENCES clickman_destinations(project_id, id) ON DELETE CASCADE,
  FOREIGN KEY (project_id, message_id) REFERENCES clickman_events(project_id, message_id) ON DELETE CASCADE
);
CREATE INDEX clickman_deliveries_due ON clickman_deliveries(project_id, destination_id, next_attempt_at) WHERE completed_at IS NULL;
