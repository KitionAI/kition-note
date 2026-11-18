```slim
body
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
  div class="a #{b} c" title='t' data=@d.e(1)
  coffee:
    square = (x) -> x * x
  javascript:
    var a = 1;
  ruby:
    x = 1
p back at top
a[href="/x" title="y"] Wrapped
  span Inside
```
