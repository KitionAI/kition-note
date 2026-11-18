```jsx
import React, { useState, useEffect } from 'react'
export default function App({ title, items = [] }) {
  const [count, setCount] = useState(0)
  useEffect(() => { document.title = `${title} (${count})` }, [count])
  return (
    <div className="app" onClick={() => setCount(count + 1)}>
      {items.map((item) => <Item key={item.id} {...item} />)}
    </div>
  )
}
```

```js
async function* paginate(url) {
  let next = url
  while (next) {
    const { data, links } = await (await fetch(next)).json()
    yield* data
    next = links?.next ?? null
  }
}
const pick = (obj, ...keys) => Object.fromEntries(keys.map((k) => [k, obj[k]]))
let a = 1, b = 'two', c = null
if (typeof window !== 'undefined' && !(a instanceof Object)) delete obj.x
label: for (let i = 0; i < 3; i++) { continue label }
new Promise((resolve, reject) => setTimeout(resolve, 10)).then(() => {}).catch(console.error)
export const config = { api: { base: process.env.API_URL || 'http://localhost:3000' } }
```

```typescript
export type Handler<E extends Event = Event> = (event: E) => void
declare module 'pkg' { export function run(): void }
abstract class Shape {
  abstract area(): number
  protected name = 'shape'
  public static from(json: string): Shape | null { return null }
}
const users = new Map<string, { id: number; tags: string[] }>()
function assert(cond: boolean, msg?: string): asserts cond {}
let tuple: [string, number] = ['a', 1]
const fn = function <T,>(x: T): T { return x }
for (const [key, value] of Object.entries(obj)) console.log(key, value)
```

```python
@app.route("/api/<int:id>", methods=["GET", "POST"])
def handler(id: int, *, verbose: bool = False) -> "Response":
    global counter
    data = request.get_json() or {}
    items = [item for item in data.get("items", []) if item]
    return jsonify({"id": id, "count": len(items)}), 200

class Config(BaseModel):
    name: str = "app"
    debug: bool = False
    def __repr__(self) -> str:
        return f"Config(name={self.name!r}, debug={self.debug})"
x = 0b1010 + 0o17 + 1_000_000 + 3j
print(r"raw\n", b'\x00', u"unicode", sep="")
```

```css
:is(h1, h2) a[href^="http"]::after { content: " ↗"; }
@media screen and (min-width: 768px) and (prefers-color-scheme: dark) {
  body { background-color: #121212; color: rgb(240 240 240 / 90%); }
}
.btn { padding: 0.5rem 1rem; transition: all .2s; z-index: 10; }
@font-face { font-family: "Inter"; src: url(inter.woff2) format("woff2"); }
nav ul li:nth-child(2n + 1) { margin-left: -4px; }
```

```bash
#!/bin/sh
readonly DIR="$(cd "$(dirname "$0")" && pwd)"
function build() {
  local target=${1:-release}
  cargo build --$target 2>/dev/null || return 1
}
while read -r line; do
  [[ "$line" =~ ^# ]] && continue
  echo "${line^^}"
done < input.txt
export NODE_ENV=production; npm run build -- --watch
```

```yaml
# app settings
app:
  name: 'My App'
  port: 8080
  ratio: 0.75
  tags: [web, api]
  owner: &owner alice
  admin: *owner
  description: >
    A folded
    paragraph
  empty:
```
