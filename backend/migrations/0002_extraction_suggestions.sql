CREATE TABLE extraction_suggestions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    inbox_item_id UUID NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('task', 'date', 'person', 'context', 'question')),
    content TEXT NOT NULL CHECK (btrim(content) <> ''),
    due_on DATE,
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (inbox_item_id, position)
);

CREATE INDEX extraction_suggestions_inbox_item_position_idx
    ON extraction_suggestions (inbox_item_id, position);

CREATE TRIGGER extraction_suggestions_set_updated_at
BEFORE UPDATE ON extraction_suggestions
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();
