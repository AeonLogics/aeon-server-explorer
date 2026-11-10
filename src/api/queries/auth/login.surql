LET $user = (SELECT * FROM ONLY user WHERE email = $email LIMIT 1);

LET $outcome = IF $user = NONE {
    crypto::argon2::compare("$argon2id$v=19$m=19456,t=2,p=1$/cTnQPy495X5RzcOJHj3Xw$0mWB1ql93AxxXfX7dHqtKlM6NsyTyPdLgLSxrttwc2M", $password);
    "bad_credentials"
} ELSE IF $user.locked_until != NONE AND $user.locked_until > time::now() {
    "locked"
} ELSE IF !crypto::argon2::compare($user.password_hash, $password) {
    LET $failed = IF $user.locked_until != NONE THEN 1 ELSE $user.failed_logins + 1 END;
    UPDATE ONLY $user.id SET
        failed_logins = $failed,
        locked_until = IF $failed >= 5 THEN time::now() + 15m ELSE NONE END
    RETURN NONE;
    IF $failed >= 5 THEN "locked_now" ELSE "bad_credentials" END
} ELSE {
    UPDATE ONLY $user.id SET
        failed_logins = 0,
        locked_until = NONE,
        last_login_at = time::now()
    RETURN NONE;
    DELETE session WHERE user = $user.id AND (expires_at < time::now() OR last_seen_at < time::now() - 30m);
    CREATE ONLY session SET
        user = $user.id,
        token_hash = crypto::sha256($session_token),
        ip = $user_ip,
        user_agent = $user_agent
    RETURN NONE;
    "ok"
};

CREATE audit SET
    action = IF $outcome = "ok" THEN "login.ok"
        ELSE IF $outcome = "locked" THEN "login.blocked"
        ELSE IF $outcome = "locked_now" THEN "login.locked"
        ELSE "login.failed" END,
    actor = $email,
    ip = $user_ip,
    user_agent = $user_agent
RETURN NONE;

RETURN $outcome;

RETURN IF $outcome = "ok" {
    {
        email: $user.email,
        must_change_password: $user.must_change_password
    }
} ELSE {
    NONE
};
