```apl
⍝ Life in APL
life ← {⊃1 ⍵ ∨.∧ 3 4 = +/ +⌿ ¯1 0 1 ∘.⊖ ¯1 0 1 ⌽¨ ⊂⍵}
avg ← {(+/⍵) ÷ ≢⍵}
x ← 3 4 ⍴ ⍳12
'it''s' , "dq" # hash comment
y ← ⍬ ⋄ z ← -x × 2.5E3
⎕IO ← 0
```

```sieve
require ["fileinto", "reject"];
# Filter mail
if header :contains "subject" "invoice" {
    fileinto "Finance";
    stop;
} elsif size :over 100K {
    reject text:
Too big.
.
;
} else { keep; }
/* block
comment */ if true { discard; }
```

```forth
\ A square word
: SQUARE ( n -- n*n ) DUP * ;
VARIABLE COUNTER
5 SQUARE . CR
: COUNTDOWN 10 0 DO I . LOOP ;
COUNTER @ 1+ COUNTER !
." Hello" S" world" TYPE
HEX FF DECIMAL 3.5 foo
' SQUARE EXECUTE
[ 1 2 + ] LITERAL
```

```factor
! Hello world in Factor
USING: io kernel math sequences ;
IN: hello
: hello ( -- ) "Hello, world!" print ;
: sq ( x -- y ) dup * ;
M: integer double 2 * ;
{ 1 2 3 } [ sq ] map .
0x1F 3.14 -7 t f
TUPLE: point x y ;
STRING: banner
multi line
;
"""triple
quoted""" drop
```

```brainfuck
++++++++[>++++[>++>+++<<-]>+<<-]>>.
read a char , and echo it .
  [-] clear
```

```eiffel
note
    description: "A counter"
class
    COUNTER
inherit
    ANY redefine default_create end
feature -- Access
    count: INTEGER
    increment
        do
            count := count + 1
        ensure
            count_increased: count = old count + 1
        end
    is_zero: BOOLEAN do Result := count = 0 and then True end
    hex: INTEGER = 0x1F
    msg: STRING = "Hello %"world%""
end
```

```turtle
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@base <http://example.org/> .
# A person
<#alice> a foaf:Person ;
    foaf:name "Alice"@en ;
    foaf:age 42 ;
    foaf:knows [ foaf:name "Bob" ] .
_:b1 foaf:mbox <mailto:b@example.org> .
:x :y "multi
line" .
```

```ntriples
<http://example.org/s> <http://example.org/p> <http://example.org/o> .
<http://example.org/s#frag> <http://example.org/p> "lit"@en .
_:b0 <http://example.org/p> "42"^^<http://www.w3.org/2001/XMLSchema#integer> .
# comment line
```
