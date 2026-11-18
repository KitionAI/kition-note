```sql
-- standard SQL through the sql mode's text/x-sql configuration
SELECT t.id, "quoted", 'it''s', x'1F', 0x2A, b'101', 0b11, 1.5e3, .5
FROM schema.table AS t WHERE t.created > DATE '2020-01-01' AND t.flag IS NOT NULL;
/* outer /* inner */ still comment */ UPDATE t SET n = n + 1, s = 'esc \' q';
INSERT INTO t (a, b) VALUES (?, ? ); BEGIN; DROP TABLE x;
SELECT COUNT(DISTINCT a) FROM t GROUP BY a HAVING COUNT(*) > 2 ORDER BY 1 DESC LIMIT 10;
SELECT a::int, b || c, d <> e, f ~ g, h ^ i, j % k FROM u.v.w @ x # y
CREATE TABLE z (id INT UNSIGNED, name VARCHAR(20), at TIMESTAMP, ok BOOLEAN DEFAULT true);
SELECT 'unclosed
string' FROM dual; SELECT unknown, null, `back`
```
