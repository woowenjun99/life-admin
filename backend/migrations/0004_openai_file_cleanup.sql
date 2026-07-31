CREATE TABLE openai_file_cleanup (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id TEXT NOT NULL UNIQUE CHECK (btrim(file_id) <> ''),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_attempt_at TIMESTAMPTZ
);

CREATE INDEX openai_file_cleanup_created_at_idx ON openai_file_cleanup (created_at);
