-- SCHEMA: audit (append-only; the app never updates or deletes these)

DEFINE TABLE OVERWRITE audit SCHEMAFULL
    PERMISSIONS NONE;

DEFINE FIELD OVERWRITE at ON audit TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE action ON audit TYPE string READONLY
    ASSERT string::len($value) >= 1 AND string::len($value) <= 64;
DEFINE FIELD OVERWRITE actor ON audit TYPE option<string> READONLY
    ASSERT $value = NONE OR string::len($value) <= 254;
DEFINE FIELD OVERWRITE ip ON audit TYPE option<string> READONLY
    ASSERT $value = NONE OR string::len($value) <= 45;
DEFINE FIELD OVERWRITE user_agent ON audit TYPE option<string> READONLY
    ASSERT $value = NONE OR string::len($value) <= 512;
DEFINE FIELD OVERWRITE detail ON audit TYPE option<string> READONLY
    ASSERT $value = NONE OR string::len($value) <= 512;
DEFINE FIELD OVERWRITE target ON audit TYPE option<string> READONLY
    ASSERT $value = NONE OR string::len($value) <= 300;
DEFINE FIELD OVERWRITE ok ON audit TYPE option<bool> READONLY;

DEFINE INDEX OVERWRITE audit_at ON audit FIELDS at;
