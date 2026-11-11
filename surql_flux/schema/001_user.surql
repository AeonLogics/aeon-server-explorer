-- SCHEMA: user (the single admin)

DEFINE TABLE OVERWRITE user SCHEMAFULL
    PERMISSIONS NONE;

DEFINE FIELD OVERWRITE email ON user TYPE string
    VALUE string::lowercase(string::trim($value))
    ASSERT string::is_email($value) AND string::len($value) <= 254;

DEFINE FIELD OVERWRITE password_hash ON user TYPE string
    ASSERT string::starts_with($value, "$argon2id$");

DEFINE FIELD OVERWRITE must_change_password ON user TYPE bool DEFAULT true;
DEFINE FIELD OVERWRITE password_changed_at ON user TYPE option<datetime>;

DEFINE FIELD OVERWRITE failed_logins ON user TYPE int DEFAULT 0 ASSERT $value >= 0;
DEFINE FIELD OVERWRITE locked_until ON user TYPE option<datetime>;
DEFINE FIELD OVERWRITE last_login_at ON user TYPE option<datetime>;

DEFINE FIELD OVERWRITE created_at ON user TYPE datetime DEFAULT time::now() READONLY;
DEFINE FIELD OVERWRITE updated_at ON user TYPE datetime VALUE time::now();

DEFINE INDEX OVERWRITE user_email ON user FIELDS email UNIQUE;
