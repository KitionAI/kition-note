```js
// Debounce a function
export function debounce(fn, wait = 300) {
  let timer = null
  return function (...args) {
    clearTimeout(timer)
    timer = setTimeout(() => fn.apply(this, args), wait)
  }
}

const sorted = [...users].sort((a, b) => a.name.localeCompare(b.name))
const total = orders.reduce((sum, { amount }) => sum + amount, 0)
document.querySelector('#app').addEventListener('click', (event) => {
  event.preventDefault()
  const target = event.target.closest('[data-id]')
  if (target?.dataset.id) open(target.dataset.id)
})
const re = /^\d{3}-\d{4}$/
const isValid = re.test(phone) && phone.length === 8 ? 'yes' : 'no'
class Counter {
  #count = 0
  static create() { return new Counter() }
  increment() { this.#count += 1; return this }
}
export { debounce as default, Counter }
module.exports = { sorted, total }
while (i < 10) { i++ }
switch (kind) { case 'a': break; default: return null }
```

```ts
type Status = 'idle' | 'loading' | 'done'
interface State<T> {
  status: Status
  data?: T
  readonly error: Error | null
}
export async function load<T>(url: string, init?: RequestInit): Promise<State<T>> {
  const res = await fetch(url, init)
  const data = (await res.json()) as T
  return { status: 'done', data, error: null }
}
function isString(x: unknown): x is string { return typeof x === 'string' }
const map: Record<string, number> = {}
enum Direction { Up = 'UP', Down = 'DOWN' }
export default class Api extends Base {
  constructor(private client: HttpClient) { super() }
}
```

```python
#!/usr/bin/env python3
"""Count words in files."""
import sys
from collections import Counter
from pathlib import Path


def count_words(path: Path) -> Counter:
    text = path.read_text(encoding="utf-8").lower()
    return Counter(word.strip(".,!?") for word in text.split())


if __name__ == "__main__":
    for arg in sys.argv[1:]:
        counts = count_words(Path(arg))
        for word, n in counts.most_common(10):
            print(f"{word:>15} {n:5d}")
    try:
        x = 1 / 0
    except ZeroDivisionError as e:
        print("error:", e, file=sys.stderr)
    finally:
        pass
squares = list(map(lambda v: v * v, range(5)))
assert isinstance(squares, list), "not a list"
total += 1; value **= 2; flag = not flag
```

```bash
# Deploy script
set -e
VERSION=$(git describe --tags)
echo "Deploying $VERSION to ${ENV:-staging}"
for host in web1 web2; do
  ssh "$host" "sudo systemctl restart app" &
done
wait
[ -d build ] && rm -rf build
tar -czf "release-$VERSION.tar.gz" dist/
curl -X POST -H "Content-Type: application/json" -d '{"ok":true}' https://example.com/hook
case "$1" in
  start) echo starting ;;
  *) exit 2 ;;
esac
```

```css
@import url("theme.css");
* { box-sizing: border-box; }
.btn:hover:not(.disabled) { transform: translateY(-1px); }
.grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
h1, h2 + p, ul > li { font-weight: 600; line-height: 1.4; color: red; }
input[type="text"] { border: 1px solid #ddd; border-radius: 8px; }
@keyframes fade { from { opacity: 0 } to { opacity: 1 } }
.card { -webkit-font-smoothing: antialiased; font: 14px/1.5 Inter, sans-serif; }
```

```html
<!-- A card -->
<section id="hero" class="hero hero--dark" data-count="3">
  <h1>Hello <em>world</em></h1>
  <img src="logo.png" alt="Logo" width="64">
  <button type="button" disabled onclick="go()">Go &rarr;</button>
  <style>.hero { color: #fff; }</style>
</section>
```

```yaml
name: CI
on:
  push:
    branches: [main]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Test
        run: |
          npm ci
          npm test
    timeout: 30
    enabled: yes
```

```sql
-- Top customers
SELECT c.name, SUM(o.total) AS spent
FROM customers AS c
INNER JOIN orders o ON o.customer_id = c.id
WHERE o.created_at BETWEEN '2024-01-01' AND '2024-12-31'
GROUP BY c.name
HAVING SUM(o.total) > 1000
ORDER BY spent DESC;
DELETE FROM sessions WHERE expires_at < NOW();
```

```json
[
  {"id": 1, "tags": ["a", "b"], "price": -12.5e3, "meta": {"draft": false}},
  {"id": 2, "tags": [], "price": 0}
]
```

```rust
use std::fmt;
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Shape { Circle(f64), Rect { w: f64, h: f64 } }
impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Shape::Circle(r) => write!(f, "circle {r}"),
            Shape::Rect { w, h } => write!(f, "rect {}x{}", w, h),
        }
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> { let s = String::from("hi"); Ok(()) }
const MAX: usize = 1_000; static NAME: &str = r"raw\path";
```

```go
package server

import (
	"net/http"
	"time"
)

type Handler func(w http.ResponseWriter, r *http.Request) error

func (s *Server) Start(addr string) error {
	srv := &http.Server{Addr: addr, ReadTimeout: 5 * time.Second}
	for _, route := range s.routes {
		http.HandleFunc(route.Path, route.Handle)
	}
	return srv.ListenAndServe()
}
```

```java
import java.util.*;

public interface Repo<T> {
    Optional<T> findById(long id);
}

@Service
public final class UserService implements Repo<User> {
    private final Map<Long, User> users = new HashMap<>();
    @Override
    public Optional<User> findById(long id) {
        return Optional.ofNullable(users.get(id));
    }
}
```

```c
typedef struct { int x; int y; } point_t;
static inline int max(int a, int b) { return a > b ? a : b; }
int main(void) {
    char buf[256];
    size_t n = sizeof(buf) / sizeof(buf[0]);
    if (fgets(buf, (int)n, stdin) != NULL) puts(buf);
    return 0;
}
```

```diff
@@ -1,3 +1,3 @@
 unchanged line
-removed line
+added line
```
