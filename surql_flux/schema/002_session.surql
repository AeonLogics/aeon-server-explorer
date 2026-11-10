-- SCHEMA: session

DEFINE TABLE OVERWRITE session SCHEMAFULL
    PERMISSIONS NONE;

DEFINE FIELD OVERWRITE user ON session TYPE record<user>
    REFERENCE ON DELETE CASCADE
    ASSERT record::exists($value);

-- SHA-256 of the cookie token, never the token itself.
DEFINE FIELD OVERWRITE token_hash ON session TYPE string
    ASSERT string::len($value) >= 32;

DEFINE FIELD OVERWRITE created_at ON session TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE last_seen_at ON session TYPE datetime DEFAULT time::now();
-- Must match MAX_AGE_SECS in src/helpers/session.rs (12 h).
DEFINE FIELD OVERWRITE expires_at ON session TYPE datetime DEFAULT time::now() + 12h READONLY;

DEFINE FIELD OVERWRITE ip ON session TYPE option<string>
    ASSERT $value = NONE OR string::len($value) <= 45;
DEFINE FIELD OVERWRITE user_agent ON session TYPE option<string>
    ASSERT $value = NONE OR string::len($value) <= 512;

DEFINE INDEX OVERWRITE session_token ON session FIELDS token_hash UNIQUE;
DEFINE INDEX OVERWRITE session_user ON session FIELDS user;
