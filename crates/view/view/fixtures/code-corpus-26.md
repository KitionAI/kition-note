```mysql
--not a comment without space
SELECT 0b101, X'FF', 1e3, a.`b`, t.*, ..x, `unclosed
SELECT 'multi
line' FROM dual WHERE \x OR \? OR @@global.v OR @"dq" OR @`bt` OR @plain.x;
SELECT ? , ?; UPDATE t SET d = TIME '10:00' + TIMESTAMP "2020";
DELIMITER // ; help; source file.sql
```

```sqlite
SELECT x'0F', 0x10, 1.e5, .5e2, a."quoted ""col""", `bt` -- c
FROM t WHERE $1 = ?2 AND :p = @q AND ?;
SELECT 'esc \' still', "unclosed
```

```cql
SELECT NaN, nan, Infinity, true FROM t WHERE a >= 1.5 AND b != 'x' -- c
UPDATE t SET m = m + {'k': 'v'} WHERE id = 0x1f; /* block */
```

```plsql
SELECT q'[quoted]', n'nat', "Quoted Id", 0x1F, b'10', .5, 1.
FROM dual -- comment
WHERE a <> b OR c != d OR e ~= f;
CREATE OR REPLACE PROCEDURE p(x IN NUMBER) IS BEGIN NULL; END p;
```

```postgresql
SELECT 'no \' escape', e'yes \' escape', E"dq", _utf8'x', n'y', .5e1, 1.;
SELECT a::text, b #> '{c}', c ?| array['d'], @-5, d ~* 'r' FROM s.t /* x /* y */ z */;
COPY t FROM STDIN; \copy t to 'f.csv'
```

```esper
@Name('q') select * from S#length(10) where x = 0x1F and y = .5 and z = b'01';
on Trigger delete from W where id = t.id -- comment
select time, 'str', "dq" from E;
```
