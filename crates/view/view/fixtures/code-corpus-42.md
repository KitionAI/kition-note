```soy
{namespace examples.simple}

/**
 * Says hello.
 * @param name The name.
 */
{template .helloName}
  {@param name: string}
  {@param? count: int}
  <div class="greeting">
    Hello {$name}! You have {$count ?: 0} items. {$unknown}
  </div>
  {if $count > 1}
    <b>many</b>
  {elseif $count == 1}
    one
  {else}
    none
  {/if}
  {for $item in $items}
    <li>{$item.name}</li>
  {/for}
  {call .other}{param x: 'str' /}{/call}
  {let $total: $count + 2.5 /}
  {msg desc="greeting"}Hi{/msg} // a comment
  {/foo}
{/template}
```
