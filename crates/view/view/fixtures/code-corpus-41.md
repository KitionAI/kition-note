```haml
%section#posts.list{class: ["a", "b"]}
  .post(data-id="1")
    %h2.post-title= post.title.upcase
    %p.meta Posted #{time_ago(post.created_at)} ago.
    = succeed "." do
      %a{href: "/more"} read more
  -# hidden
    nested hidden line
  %script{type: "text/javascript"}
    :javascript
      alert("hi");
  %p{style: "color: red"}: inline colon
  %br/
  %input{type: :text, value: @value, disabled: true}
  != raw_html
  ~ preserve_this
/[if IE]
  %p ie only
%span.a.b.c text with . dots and #hash
```
