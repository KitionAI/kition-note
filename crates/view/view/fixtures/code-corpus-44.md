```slim
doctype html
html
  head
    title = @page_title
    meta name="keywords" content=keywords
    javascript:
      alert('Slim supports embedded javascript!')
  body
    h1#headline.title Markup examples
    #content
      p This example shows you how a basic Slim file looks.
      == yield
      - if items.any?
        table#items
          - for item in items
            tr
              td.name = item.name
              td.price = item.price
      - else
        p No items found. Please add some inventory.
    div id="footer" class=footer_class
      | Copyright &copy; #{year} #{company_name}
    / a comment
      under it
    a href="/x" data-id=@id Link
    input type="text" disabled=true value=(@user.name)
    ul: li: a href="#" Nested
    <div>raw html</div>
```
