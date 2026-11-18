```js
import { readFile } from 'node:fs/promises'
const answer = 42, hex = 0xff, pi = 3.14
let list = [1, 2, 3].map((x) => x * 2)
class Point extends Base {
  constructor(x, y) { super(); this.x = x; this.y = y }
  get length() { return Math.sqrt(this.x ** 2) }
}
function greet(name, options = {}) {
  // say hello
  /* block */ if (name === null || !options.loud) return `Hi ${name.trim()}!`
  console.log("done", true, undefined, /ab+c/g)
}
export default async function main() { await greet('a') }
```

```ts
interface User { id: number; name?: string }
type Id = string | number
export function find(users: User[], id: Id): User | undefined {
  const map = new Map<number, User>()
  return users.find((u) => u.id === id) as User
}
enum Color { Red = 1, Green }
```

```python
import os
from typing import List

@dataclass
class Point(Base):
    """A point."""
    def __init__(self, x: int, y=0):
        self.x = x  # set
        print(len(items), True, None, 3.5e2, 0x1f)
        return f"{x} and {self.y!r}" + 'single' + b"bytes"

def greet(name):
    if name is not None and name != "":
        yield lambda a, *args, **kw: a
```

```rust
use std::collections::HashMap;
/// Docs
#[derive(Debug, Clone)]
pub struct Point<'a> { x: f64, name: &'a str }
impl Point<'_> {
    pub fn new(x: f64) -> Self { let v: Vec<u8> = vec![1, 2]; println!("{x}"); Self { x, name: "p" } }
}
fn main() { let mut m = HashMap::new(); m.insert(1u32, 'c'); if true { return; } }
```

```go
package main

import "fmt"

type Point struct { X, Y int }

func (p *Point) Add(o Point) Point {
	s := []int{1, 2, 3}
	fmt.Println("sum", len(s), nil, true, 0.5)
	return Point{p.X + o.X, p.Y + o.Y}
}
```

```c
#include <stdio.h>
#define MAX 10
int main(int argc, char **argv) {
    unsigned long n = 0UL; /* count */
    for (int i = 0; i < MAX; i++) { printf("%d\n", i); }
    return NULL == argv ? -1 : 0;
}
```

```java
package demo;
import java.util.List;
@Override
public class App extends Base implements Runnable {
    private static final int MAX = 10;
    public void run() { String s = "x"; List<String> l = new ArrayList<>(); if (s != null) { this.go(1.5f, 'c', true); } }
}
```

```bash
#!/bin/bash
NAME="world"
for f in *.txt; do
  echo "Hello $NAME ${f%.txt}" | grep -v foo > /dev/null 2>&1
  if [ -f "$f" ]; then cat $f; fi
done
export PATH=$HOME/bin:$PATH # comment
```

```sql
SELECT u.id, COUNT(*) AS total FROM users u
LEFT JOIN orders o ON o.user_id = u.id
WHERE u.name LIKE 'a%' AND o.amount > 10.5 -- note
GROUP BY u.id ORDER BY total DESC LIMIT 5;
```

```css
/* card */
.card > a:hover, #main {
  color: #5645d4;
  margin: 0 auto 12px;
  font-family: "Inter", sans-serif !important;
}
@media (max-width: 600px) { .card { display: none } }
```

```json
{"name": "kit", "version": 1.5, "ok": true, "none": null, "list": [1, "a"]}
```

```yaml
# config
name: kition
version: 1.2
enabled: true
items:
  - "a"
  - b: 3
```

```html
<!DOCTYPE html>
<div class="box" id='x'>Text &amp; more<br/></div>
<!-- comment -->
```
