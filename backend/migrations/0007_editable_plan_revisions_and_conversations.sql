ALTER TABLE plans
    ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0);

CREATE TABLE plan_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id UUID NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision > 0),
    source TEXT NOT NULL CHECK (source IN ('initial', 'manual', 'step_status', 'assistant')),
    snapshot JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (plan_id, revision)
);

CREATE TABLE plan_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id UUID NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content TEXT NOT NULL CHECK (btrim(content) <> ''),
    proposal JSONB,
    base_revision INTEGER,
    applied_revision INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (role = 'user' AND proposal IS NULL AND base_revision IS NULL AND applied_revision IS NULL)
        OR (role = 'assistant' AND (proposal IS NULL OR base_revision IS NOT NULL))
    )
);

CREATE INDEX plan_revisions_plan_id_revision_idx ON plan_revisions (plan_id, revision DESC);
CREATE INDEX plan_messages_plan_id_created_at_idx ON plan_messages (plan_id, created_at DESC, id DESC);

INSERT INTO plan_revisions (plan_id, revision, source, snapshot)
SELECT
    p.id,
    p.revision,
    'initial',
    jsonb_build_object(
        'summary', p.summary,
        'status', p.status,
        'steps', COALESCE(
            jsonb_agg(
                jsonb_build_object(
                    'id', s.id,
                    'position', s.position,
                    'title', s.title,
                    'rationale', s.rationale,
                    'status', s.status,
                    'dueOn', s.due_on,
                    'waitingOn', s.waiting_on,
                    'isNextAction', s.is_next_action
                )
                ORDER BY s.position
            ) FILTER (WHERE s.id IS NOT NULL),
            '[]'::jsonb
        )
    )
FROM plans p
LEFT JOIN plan_steps s ON s.plan_id = p.id
GROUP BY p.id;
