BEGIN TRANSACTION;

IF (SELECT VALUE id FROM ONLY user LIMIT 1) != NONE {
    THROW "admin_exists"
};

CREATE ONLY user SET
    email = $email,
    password_hash = crypto::argon2::generate($password),
    must_change_password = true
RETURN NONE;

CREATE audit SET action = "admin.created", actor = $email RETURN NONE;

COMMIT TRANSACTION;
