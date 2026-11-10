LET $current = (
    SELECT * FROM ONLY session
    WHERE token_hash = crypto::sha256($session_token)
        AND expires_at > time::now()
        AND last_seen_at > time::now() - 30m
    LIMIT 1
);

LET $user = IF $current = NONE { NONE } ELSE { (SELECT * FROM ONLY $current.user) };

IF $touch AND $current != NONE AND $current.last_seen_at < time::now() - 1m {
    UPDATE ONLY $current.id SET last_seen_at = time::now() RETURN NONE;
};

RETURN IF $user = NONE {
    NONE
} ELSE {
    {
        email: $user.email,
        must_change_password: $user.must_change_password
    }
};
