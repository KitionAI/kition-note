```pegjs
// Simple arithmetic grammar
start
  = additive

additive
  = left:multiplicative "+" right:additive { return left + right; }
  / multiplicative

multiplicative
  = left:primary "*" right:multiplicative { return left * right; }
  / primary

primary
  = integer
  / "(" additive:additive ")" { return additive; }

integer "integer"
  = digits:[0-9]+ { return parseInt(digits.join(""), 10); }
/* block
comment */ _ = [ \t\n\r]*
```

```smarty
{* Smarty template *}
<h1>{$title|upper}</h1>
{if $user.loggedIn}
  Welcome, {$user.name|escape:'html'}!
{elseif $count > 5}
  Many: {$count + 1}
{else}
  {include file="login.tpl" title="Sign in"}
{/if}
{foreach from=$items item=i key=k}
  <li>{$k}: {$i@index} {$i|truncate:30}</li>
{/foreach}
{literal}<script>var a = {b: 1};</script>{/literal}
```
