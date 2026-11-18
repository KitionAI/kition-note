```jade
extends ../layout
block append scripts
  script(src="/app.js" defer)
mixin list(items)
  ul
    each val, key in {a: 1, b: 2}
      li= key + ': ' + val
+list(['x', 'y'])
p
  | Hello #{name}!
  != unescaped
  strong.big#hero(data-x={a: [1, 2]}, onclick="go()") Click
- if (n > 1)
  span= n * 2
unless loggedIn
  a(href="/login") Log in
else if admin
  p admin
while i < 3
  p= i++
script(type="text/x-template").
  <b>not js</b>
:css
  .a { color: red }
p: em nested
div&attributes({id: 'z'})
//
  a block comment
  over lines
yield
for x in xs
  = x
```
