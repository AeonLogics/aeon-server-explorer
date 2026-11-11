CREATE audit SET
    action = $action,
    actor = $actor,
    target = $target,
    ip = $user_ip,
    user_agent = $user_agent,
    detail = $detail,
    ok = $ok
RETURN NONE;
