```asterisk
[macro-stdexten]
exten => s,1,NoOp(Start)
 same => n,GotoIf($["${ARG1}" = ""]?bad)
exten => 300 bad line
switch => Realtime/@
ignorepat => 9
'single quoted' "double
; plain comment ;--- not block
#exec /bin/date
```

```dtd
<!DOCTYPE note [
  <!ELEMENT a (b, c)?>
  <!ATTLIST a href CDATA #IMPLIED>
  <!ENTITY % param "x">
]>
<!-- multi
line -->
<? pi ?>
'single'
```

```velocity
#set( $list = ["a", 'b', 3.5] )
#if( $a && !$b || $c == 'x' )yes#end
$customer.Address.getCity()
#{if}($x)y#{end}
#mymacro("arg")
<a href="$url">$link.text</a>
#include( "header.vm" )
##
#[[
unparsed ]]#
#break
```

```tiddlywiki
|!Header|!Header 2|
;term
:definition
<<<
quote block
<<<
//{{{
code
//}}}
~WikiWord [[x]] ]] @@highlight@@ -- dash
file://local/path ftp://host
<<slider chk Target "label">>
'' unclosed bold
```

```tiki
{img src="a.png" width=100}
{DIV(class=note)}content{DIV}
+ plus item
((Page|alias)) [[not]] __a__b__
::start
end::
^box
continues^
!!!!!five
```
