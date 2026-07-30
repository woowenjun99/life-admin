CREATE TABLE plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    inbox_item_id UUID NOT NULL UNIQUE REFERENCES inbox_items(id) ON DELETE RESTRICT,
    summary TEXT NOT NULL CHECK (btrim(summary) <> ''),
    status TEXT NOT NULL CHECK (status IN ('ready', 'waiting', 'complete')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE plan_steps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id UUID NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    title TEXT NOT NULL CHECK (btrim(title) <> ''),
    rationale TEXT NOT NULL CHECK (btrim(rationale) <> ''),
    status TEXT NOT NULL CHECK (status IN ('ready', 'waiting', 'complete')),
    due_on DATE,
    waiting_on TEXT,
    is_next_action BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (status = 'waiting' AND waiting_on IS NOT NULL AND btrim(waiting_on) <> '')
        OR (status <> 'waiting' AND waiting_on IS NULL)
    ),
    CHECK (NOT is_next_action OR status = 'ready'),
    UNIQUE (plan_id, position)
);

CREATE UNIQUE INDEX plan_steps_one_next_action_per_plan_idx
    ON plan_steps (plan_id)
    WHERE is_next_action;
CREATE INDEX plans_inbox_item_idx ON plans (inbox_item_id);
CREATE INDEX plan_steps_plan_id_position_idx ON plan_steps (plan_id, position);

CREATE TRIGGER plans_set_updated_at
BEFORE UPDATE ON plans
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER plan_steps_set_updated_at
BEFORE UPDATE ON plan_steps
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();
