CREATE TABLE fcm_registration_tokens (
    token TEXT PRIMARY KEY CHECK (btrim(token) <> ''),
    owner_uid TEXT NOT NULL CHECK (btrim(owner_uid) <> ''),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX fcm_registration_tokens_owner_uid_idx
    ON fcm_registration_tokens (owner_uid);

CREATE TRIGGER fcm_registration_tokens_set_updated_at
BEFORE UPDATE ON fcm_registration_tokens
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

-- A date-only due reminder is sent once per Plan step. A failed delivery is
-- retried after the short lease; a successful or no-token delivery is marked
-- sent so a long-running worker cannot repeatedly notify a person.
CREATE TABLE due_fcm_notification_claims (
    plan_step_id UUID NOT NULL REFERENCES plan_steps(id) ON DELETE CASCADE,
    due_on DATE NOT NULL,
    owner_uid TEXT NOT NULL CHECK (btrim(owner_uid) <> ''),
    plan_id UUID NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at TIMESTAMPTZ,
    PRIMARY KEY (plan_step_id, due_on)
);

CREATE INDEX due_fcm_notification_claims_retry_idx
    ON due_fcm_notification_claims (claimed_at)
    WHERE sent_at IS NULL;
