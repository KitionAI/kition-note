```pug
doctype html
html(lang="en")
  head
    title= pageTitle
    script(type='text/javascript').
      if (foo) { bar(1 + 5) }
    style.
      body { color: red; }
  body
    //- a hidden comment
    // a visible comment
    h1#main.title Pug - node template engine
    #container.col
      if youAreUsingPug
        p You are amazing
      else
        p Get on it!
      p.
        Pug is a terse and simple templating language with a
        strong focus on performance and powerful features.
    - var items = ['a', 'b', 'c']
    ul
      each item, i in items
        li(class=item, data-index=i)= item
    a(href='/x' + id, target="_blank" checked) Link #{user.name} here
    +card('Title', {big: true})
    mixin card(title, opts)
      .card= title
    case n
      when 0: p zero
      default: p many
    | plain text line
    <div>raw html</div>
    include partials/footer.pug
    extends layout
    block content
    input(type="checkbox" checked=false disabled)
```
