```mysql
-- MySQL schema
CREATE TABLE `users` (
  id INT UNSIGNED NOT NULL AUTO_INCREMENT,
  email VARCHAR(255) DEFAULT "none",
  created DATETIME DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
SELECT u.id, COUNT(*) AS n FROM users u WHERE u.email LIKE '%@x.com' GROUP BY u.id;
SET @total = 0x1F + b'1010' + 3.5e2 + .5 + 1.;
# hash comment
SELECT _utf8'text', N'national', DATE '2024-01-01', \N, `odd``name`;
/* block /* nested */ still */ UPDATE t SET a = a + 1;
SELECT @@session.sql_mode, @'quoted var';
```

```sqlite
CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY AUTOINCREMENT, body TEXT);
INSERT INTO notes (body) VALUES (?1), (:name), ($val), (@x);
SELECT "quoted col", datetime('now') FROM notes WHERE id IN (1, 2) LIMIT 5;
.tables
PRAGMA foreign_keys = ON;
```

```cql
CREATE KEYSPACE ks WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 3};
CREATE TABLE ks.users (id uuid PRIMARY KEY, name text, tags set<text>);
SELECT * FROM ks.users WHERE id = 5 ALLOW FILTERING; // comment
INSERT INTO users (id, name) VALUES (uuid(), 'bob') USING TTL 86400;
```

```plsql
DECLARE
  v_count NUMBER := 0;
  v_name VARCHAR2(30) := N'Ann';
BEGIN
  SELECT COUNT(*) INTO v_count FROM employees WHERE dept_id = 10;
  IF v_count > 0 THEN
    DBMS_OUTPUT.PUT_LINE('Found: ' || TO_CHAR(v_count));
  END IF;
EXCEPTION WHEN NO_DATA_FOUND THEN NULL;
END;
/
```

```postgresql
CREATE OR REPLACE FUNCTION add(a integer, b integer) RETURNS integer AS $$
  SELECT a + b;
$$ LANGUAGE sql IMMUTABLE;
SELECT E'esc\'aped', 'it''s', x'1F', B'101', '{1,2}'::int[] @> ARRAY[1];
SELECT jsonb_build_object('k', 1) ->> 'k' FROM t WHERE ts > TIMESTAMP '2020-01-01';
```

```esper
select symbol, avg(price) as avgPrice from StockTick#time(30 sec)
where price > 10.5 group by symbol having count(*) > 2 output every 5 seconds;
insert into Alerts select * from pattern [every a=Login -> b=Logout];
```
