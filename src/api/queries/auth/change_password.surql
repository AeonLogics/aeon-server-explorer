LET $current = (
    SELECT * FROM ONLY session
    WHERE token_hash = crypto::sha256($session_token)
        AND expires_at > time::now()
        AND last_seen_at > time::now() - 30m
    LIMIT 1
);

LET $user = IF $current = NONE { NONE } ELSE { (SELECT * FROM ONLY $current.user) };

LET $outcome = IF $user = NONE {
    "unauthorized"
} ELSE IF !crypto::argon2::compare($user.password_hash, $current_password) {
    "wrong_current"
} ELSE IF crypto::argon2::compare($user.password_hash, $new_password) {
    "same"
} ELSE {
    UPDATE ONLY $user.id SET
        password_hash = crypto::argon2::generate($new_password),
        must_change_password = false,
        password_changed_at = time::now()
    RETURN NONE;
    DELETE session WHERE user = $user.id AND id != $current.id;
    "ok"
};

IF $user != NONE {
    CREATE audit SET
        action = IF $outcome = "ok" THEN "password.changed" ELSE "password.change_failed" END,
        actor = $user.email,
        ip = $user_ip,
        user_agent = $user_agent
    RETURN NONE;
};

RETURN $outcome;
