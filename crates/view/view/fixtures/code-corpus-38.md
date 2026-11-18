```pegjs
{
  function make(kind, value) { return { kind: kind, value: value }; }
}
string "string"
  = '"' chars:[^"\\]* '"' { return make("str", chars.join('')); }
escape = "\\" c:[nrt\]] { return c; }
list = head:item tail:("," item)* {
  var out = [head];
  for (var i = 0; i < tail.length; i++) { out.push(tail[i][1]); }
  return out;
}
item = $[a-zA-Z_]+ / &"x" !"y" .
```

```smarty
{assign var="x" value=$a.b.c}
{$foo|default:"none"|lower} {$arr[0]} {$obj->prop}
{section name=i loop=$list}{$list[i]}{/section}
{* multi
line comment *}
{if $a == 1 && $b != 'q' || !$c}ok{/if}
{"unterminated
string"} plain {ldelim}
{function name=menu level=0}{/function}
{debug}
```
