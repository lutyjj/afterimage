-- A personal API token: what a script or an agent signs in with. Like a session,
-- the holder keeps an opaque random secret and the database only its SHA-256;
-- unlike one, it has a name and lasts until it is revoked.
CREATE TABLE access_tokens (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_hash BLOB NOT NULL UNIQUE,
    -- 1 when the token may only read: the operations the model marks @readonly.
    read_only INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    -- The last request that used it; NULL until one did.
    last_used_at INTEGER
) STRICT;

CREATE INDEX access_tokens_by_account ON access_tokens (account_id);
