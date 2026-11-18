```slim
ruby:
  greeting = "hello"
css:
  .a { color: red; }
body
  nav.menu: ul.list
    li.item: a href=link_to(path, class: "x") Home
  .card[data-id="1" class="c"] Text after wrapper
  section(id="s" role=main_role)
  p
    | Multi-line text
      continues here with #{interpolated} value
  = render 'partial', locals: { a: 1,
    b: 2 }
  - items.each do |item|
    span.item = item
  p == "raw #{x}"
  img src=image_url alt=""/
  input checked=false name=:sym
  '  trailing text
  meta charset="utf-8"
  p.x-large.-neg#id-1 Shortcuts
  ' text with quote
  coffee:
    square = (x) -> x * x
p back at top
```
