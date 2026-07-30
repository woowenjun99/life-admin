CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TABLE inbox_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_uid TEXT NOT NULL CHECK (btrim(owner_uid) <> ''),
    source_type TEXT NOT NULL CHECK (source_type IN ('text', 'image', 'pdf')),
    original_text TEXT,
    original_filename TEXT,
    content_type TEXT,
    storage_key TEXT,
    byte_size BIGINT,
    status TEXT NOT NULL DEFAULT 'captured'
        CHECK (status IN ('captured', 'reviewing', 'planned', 'archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (
            source_type = 'text'
            AND original_text IS NOT NULL
            AND btrim(original_text) <> ''
            AND original_filename IS NULL
            AND content_type IS NULL
            AND storage_key IS NULL
            AND byte_size IS NULL
        )
        OR (
            source_type = 'image'
            AND original_text IS NULL
            AND original_filename IS NOT NULL
            AND btrim(original_filename) <> ''
            AND content_type IN ('image/jpeg', 'image/png')
            AND storage_key IS NOT NULL
            AND btrim(storage_key) <> ''
            AND byte_size > 0
        )
        OR (
            source_type = 'pdf'
            AND original_text IS NULL
            AND original_filename IS NOT NULL
            AND btrim(original_filename) <> ''
            AND content_type = 'application/pdf'
            AND storage_key IS NOT NULL
            AND btrim(storage_key) <> ''
            AND byte_size > 0
        )
    )
);

CREATE INDEX inbox_items_owner_created_at_idx
    ON inbox_items (owner_uid, created_at DESC);
CREATE INDEX inbox_items_owner_status_created_at_idx
    ON inbox_items (owner_uid, status, created_at DESC);
CREATE UNIQUE INDEX inbox_items_storage_key_idx
    ON inbox_items (storage_key)
    WHERE storage_key IS NOT NULL;

CREATE TRIGGER inbox_items_set_updated_at
BEFORE UPDATE ON inbox_items
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();
