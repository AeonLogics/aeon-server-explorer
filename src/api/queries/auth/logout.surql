LET $current = (SELECT * FROM ONLY session WHERE token_hash = crypto::sha256($session_token) LIMIT 1);

IF $current != NONE {
    CREATE audit SET
        action = "logout",
        actor = $current.user.email,
        ip = $user_ip,
        user_agent = $user_agent
    RETURN NONE;
};

DELETE session WHERE token_hash = crypto::sha256($session_token);
