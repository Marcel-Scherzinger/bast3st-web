DROP VIEW IF EXISTS active_specifications;
CREATE VIEW active_specifications AS (
    WITH cte AS (
        SELECT
            c.slotid,
            max(c.generation) AS active_gen
        FROM specifications AS c
        WHERE c.active
        GROUP BY c.slotid
    )

    SELECT
        c.specid,
        s.userid,
        c.slotid,
        u.username,
        s.slotname,
        c.generation,
        c.content
    FROM specifications AS c
    INNER JOIN slots AS s ON c.slotid = s.slotid
    INNER JOIN users AS u ON s.userid = u.userid
    INNER JOIN cte AS a ON s.slotid = a.slotid AND c.generation = a.active_gen
);
