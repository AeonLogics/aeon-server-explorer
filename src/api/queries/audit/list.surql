SELECT
    <string> id AS id,
    <string> at AS at,
    action,
    actor,
    target,
    ip,
    detail,
    ok
FROM audit
ORDER BY at DESC
LIMIT 300;
