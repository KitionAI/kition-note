```soy
{namespace a.b}
import {Foo as Bar} from 'path/to/file.soy';
{template .list kind="html"}
  {@param items: list<[name: string, id: int]>}
  {@inject? flag: bool}
  {@state open: bool = false}
  <ul id="{$flag ? 'x' : 'y'}">
  {for $i in $items}
    <li data-id="{$i.id}">{$i.name |escapeHtml}</li>
  {ifempty}
    <li>none</li>
  {/for}
  </ul>
  {let $m: map('a': 1, 'b': [1, 2]) /}
  {let $r: record(x: 1, y: 'z') /}
  {let $l: [$x * 2 for $x in [1, 2, 3]] /}
  {switch $flag}{case true}t{default}f{/switch}
  {literal}{not a command}{/literal}
  <{Bar(x: 1)} />
  /* block
  comment */ {sp}{nil}
{/template}
{template .styles kind="css"}
  .a { color: red; }
{/template}
```
