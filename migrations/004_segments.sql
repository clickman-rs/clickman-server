CREATE TABLE clickman_segments (
 id uuid PRIMARY KEY, project_id uuid NOT NULL REFERENCES clickman_projects(id) ON DELETE CASCADE,
 key text NOT NULL, definition jsonb NOT NULL, enabled boolean NOT NULL DEFAULT true,
 fire_existing boolean NOT NULL DEFAULT false, revision integer NOT NULL DEFAULT 1,
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), evaluated_at timestamptz,
 UNIQUE(project_id,key), UNIQUE(project_id,id)
);
CREATE TABLE clickman_segment_members (
 project_id uuid NOT NULL, segment_id uuid NOT NULL, actor_id bigint NOT NULL,
 present boolean NOT NULL, entries integer NOT NULL DEFAULT 1, revision integer NOT NULL,
 entered_at timestamptz NOT NULL, evaluated_at timestamptz NOT NULL,
 PRIMARY KEY(project_id,segment_id,actor_id),
 FOREIGN KEY(project_id,segment_id) REFERENCES clickman_segments(project_id,id) ON DELETE CASCADE,
 FOREIGN KEY(project_id,actor_id) REFERENCES clickman_actors(project_id,id) ON DELETE CASCADE
);
CREATE INDEX clickman_segment_present ON clickman_segment_members(project_id,segment_id) WHERE present;
CREATE TABLE clickman_segment_entries (
 id uuid PRIMARY KEY,project_id uuid NOT NULL,segment_id uuid NOT NULL,actor_id bigint NOT NULL,
 ordinal integer NOT NULL, revision integer NOT NULL, payload jsonb NOT NULL, created_at timestamptz NOT NULL,
 UNIQUE(project_id,segment_id,actor_id,revision,ordinal),UNIQUE(project_id,id),
 FOREIGN KEY(project_id,segment_id,actor_id) REFERENCES clickman_segment_members(project_id,segment_id,actor_id) ON DELETE CASCADE
);
CREATE TABLE clickman_webhooks (
 id uuid PRIMARY KEY,project_id uuid NOT NULL,segment_id uuid NOT NULL,name text NOT NULL,
 kind text NOT NULL CHECK(kind IN ('webhook','zapier')),config jsonb NOT NULL, enabled boolean NOT NULL DEFAULT true,
 UNIQUE(project_id,id),UNIQUE(project_id,segment_id,name),
 FOREIGN KEY(project_id,segment_id) REFERENCES clickman_segments(project_id,id) ON DELETE CASCADE
);
CREATE TABLE clickman_webhook_deliveries (
 id uuid PRIMARY KEY,project_id uuid NOT NULL,webhook_id uuid NOT NULL,entry_id uuid NOT NULL,
 status text NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','delivered','failed','canceled')),
 attempts integer NOT NULL DEFAULT 0,next_attempt_at timestamptz NOT NULL DEFAULT now(),last_status integer,
 last_error text,completed_at timestamptz,created_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(project_id,webhook_id,entry_id),
 FOREIGN KEY(project_id,webhook_id) REFERENCES clickman_webhooks(project_id,id) ON DELETE CASCADE,
 FOREIGN KEY(project_id,entry_id) REFERENCES clickman_segment_entries(project_id,id) ON DELETE CASCADE
);
CREATE INDEX clickman_webhook_due ON clickman_webhook_deliveries(project_id,next_attempt_at) WHERE status='pending';
