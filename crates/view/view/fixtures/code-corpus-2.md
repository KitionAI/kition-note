```javascript
const express = require('express')
const app = express()
app.get('/users/:id', async (req, res) => {
  const { id } = req.params
  try {
    const user = await db.find({ id: Number(id), active: true })
    if (!user) return res.status(404).json({ error: 'not found' })
    res.json(user)
  } catch (err) {
    console.error(err.message)
  }
})
for (const item of items) { total += item.price * item.qty }
let count = 0; count++; const half = count / 2
const obj = { name, age: 30, greet() { return this.name }, ...rest }
setTimeout(() => console.log(`done in ${Date.now() - start}ms`), 100)
```

```typescript
import type { Request } from './types'
export class Store<T> implements Iterable<T> {
  private items: T[] = []
  constructor(private readonly name: string) {}
  add(item: T): void { this.items.push(item) }
  get size(): number { return this.items.length }
}
const handler = (req: Request): Promise<string> => fetch(req.url).then((r) => r.text())
let value = input as unknown as number
```

```py
class Stack:
    def __init__(self):
        self.items = []

    def push(self, item):
        self.items.append(item)

numbers = [x ** 2 for x in range(10) if x % 2 == 0]
with open("data.txt", "r") as f:
    data = f.read().splitlines()
result = {"a": 1, 'b': [1, 2]}
async def fetch(url: str) -> dict:
    raise ValueError(f"bad {url}")
```

```rs
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adds() {
        let total: i32 = vec![1, 2, 3].iter().sum();
        assert_eq!(total, 6, "sum is {}", total);
        match Some(3) { Some(n) if n > 2 => println!("big"), _ => {} }
    }
}
impl<T: Clone> From<T> for Wrapper<T> { fn from(v: T) -> Self { Wrapper(v) } }
```

```go
func main() {
	ch := make(chan int, 10)
	go func() { ch <- 42 }()
	if v, ok := <-ch; ok {
		fmt.Printf("%d\n", v)
	}
	var m = map[string]int{"a": 1}
	defer close(ch)
}
```

```cpp
#include <vector>
template <typename T>
class Box {
public:
    explicit Box(T v) : value(v) {}
    T get() const { return value; }
private:
    T value;
};
std::vector<int> nums = {1, 2, 3};
auto it = std::find(nums.begin(), nums.end(), 2);
```

```java
public class Main {
    public static void main(String[] args) {
        int sum = 0;
        for (int i = 0; i < args.length; i++) {
            sum += Integer.parseInt(args[i]);
        }
        System.out.println("Sum: " + sum);
    }
}
```

```sh
#!/usr/bin/env bash
set -euo pipefail
if [[ -z "${1:-}" ]]; then
  echo "usage: $0 <file>" >&2
  exit 1
fi
cp "$1" /tmp/ && ls -la /tmp | wc -l
npm install --save-dev typescript
```

```sql
CREATE TABLE users (id INTEGER PRIMARY KEY, name VARCHAR(100) NOT NULL, created_at TIMESTAMP);
INSERT INTO users (name) VALUES ('Ada'), ("Bob");
UPDATE users SET name = 'Eve' WHERE id = 3;
SELECT name FROM users WHERE created_at > '2024-01-01' AND id IN (1, 2, 3);
```

```css
:root { --accent: #5645d4; }
body {
  margin: 0;
  background: rgba(255, 255, 255, 0.9) url("bg.png") no-repeat;
  transition: opacity 0.2s ease-in-out;
  color: var(--accent);
}
a::before { content: "→"; }
```

```json
{
  "compilerOptions": {
    "target": "es2022",
    "strict": true,
    "paths": { "@/*": ["src/*"] }
  }
}
```

```yml
version: "3.8"
services:
  web:
    image: nginx:latest
    ports:
      - "80:80"
    environment:
      DEBUG: false
```

```html
<html lang="en">
<head><meta charset="utf-8"><title>Demo</title></head>
<body class=main>
  <a href="https://kition.ai" target="_blank">Kition</a>
  <script>const x = 1; console.log(x)</script>
</body>
</html>
```

```kotlin
data class User(val name: String, var age: Int = 0)
fun greet(user: User): String = "Hello, ${user.name}"
```

```csharp
public class Program {
    static void Main(string[] args) { var list = new List<int>(); Console.WriteLine("Hi"); }
}
```

```toml
[package]
name = "kition"
version = "0.1.0"
```
