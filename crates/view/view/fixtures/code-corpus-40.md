```haml
!!! 5
%html
  %head
    %title= @title
    %link{rel: "stylesheet", href: "/app.css"}
  %body
    / A comment
      indented under the comment
    #main.container
      %h1.title Welcome, #{@user.name}!
      %p(class="lead" id='intro') Some text here.
      - if @items.any?
        %ul
          - @items.each do |item|
            %li= link_to item.name, item_path(item)
      - else
        %p No items.
      = render partial: "footer",
        locals: { year: 2024 }
      -# a silent comment
      <div class="raw">html &amp; more</div>
      %a{href: "#", data: {id: 1}} Link
```
