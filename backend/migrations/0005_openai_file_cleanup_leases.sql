ALTER TABLE openai_file_cleanup
    ADD COLUMN lease_token UUID,
    ADD COLUMN lease_expires_at TIMESTAMPTZ;

CREATE INDEX openai_file_cleanup_retry_idx
    ON openai_file_cleanup (last_attempt_at ASC NULLS FIRST, created_at ASC)
    WHERE lease_expires_at IS NULL;

CREATE INDEX openai_file_cleanup_lease_expiry_idx
    ON openai_file_cleanup (lease_expires_at)
    WHERE lease_expires_at IS NOT NULL;
