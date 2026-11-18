```asterisk
; Asterisk dialplan
[globals]
TRUNK=SIP/provider
[default]
exten => 100,1,Answer()
same => n,Playback(hello-world)
same => n,Hangup()
exten => _9X.,1,Dial(${TRUNK}/${EXTEN:1},30)
include => other-context
#include "extra.conf"
;-- block
comment --;
exten = 200,1,Set(VAR="value")
```

```dtd
<?xml version="1.0"?>
<!-- A DTD -->
<!ELEMENT note (to, from, heading?, body+)>
<!ELEMENT to (#PCDATA)>
<!ATTLIST note id ID #REQUIRED type CDATA "memo">
<!ENTITY copy "&#169;">
<!ELEMENT list (item | sub)*>
```

```velocity
## A comment
#set($name = "World")
Hello $name! You have $mailbox.count messages.
#if($user.isAdmin())
  <b>Admin</b>
#elseif($count > 5)
  many
#else
  none
#end
#foreach($item in $items)
  $velocityCount: $item.name ($foreach.count)
#end
#* block
comment *#
#macro(greet $who) Hi $who #end
#[[ raw $text ]]#
$!{optional} 42 "lone quote
```

```tiddlywiki
!Heading
* bullet item
# numbered
''bold'' //italic// __under__ --strike-- text
[[Link Title|Target]] http://example.com/path @tag
{{{inline code}}} and <<today>> macro <<tag Foo>>
{{{
code block
}}}
/***
comment block
***/
----
> quoted
|cell|cell|
123 "quote ~tilde
```

```tiki
!! Heading two
!Heading one
* item one
# numbered
__bold__ ''italic'' ((WikiPage)) [http://x.com|link]
-=title=- --deleted-- ===under=== ^boxed^
{CODE(colors=>php caption="Hi")}echo 1;{CODE}
||a|b|| ::centered:: ~np~raw~/np~
plain text
```
