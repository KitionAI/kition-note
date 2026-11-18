```apl
fib←{⍵≤1:⍵ ⋄ (∇⍵-1)+∇⍵-2}
m ← 2 3⍴⍳6 ⋄ +/[1] m ⍝ sum columns
v ← ¯3.5 1E¯2 0J1 ,⍥⊂ 'a\'b'
⍉⌽⊖ m ∘.× m
z ← ⍬≡⍬ ⋄ ⎕← 'out'
"open
```

```sieve
require "vacation";
if allof (address :is "from" "a@b.c", not exists "x-spam") {
  vacation :days 7 "Away";
} elsif anyof (true, false) { redirect "x@y.z"; }
if header :matches "Subject" "*\"quoted\"*" { addflag "\\Seen"; }
reject text:
  # not a comment at start
line two
.
setflag "Flagged";
/* one */ /* two
three */ keep;
size :under 1M
```

```forth
: STAR 42 EMIT ;   : STARS 0 ?DO STAR LOOP ;
5 STARS CR
CONSTANT TEN  10 TO TEN
: test [ 1 ] literal ; immediate
0x10 0b101 -12 +7 1e3 .5 Infinity nan 1.2.3
POSTPONE foo
:NONAME 1 + ;
.( paren string) ( a comment ) C" counted"
2VARIABLE PAIR variable lower
: ;
```

```factor
USE: math.functions
:: fib ( n -- m ) n 2 < [ n ] [ n 1 - fib n 2 - fib + ] if ;
GENERIC: area ( shape -- n )
<PRIVATE
: helper ( -- ) ;
PRIVATE>
1 2 3 4 + "a\"b" "two
lines" length
#! shebang style
>upper >string? foo* bar> (baz) T{ point } H{ } V{ }
3.5x 12 0b101 0o17 -1.5
SYMBOL: thing
```

```brainfuck
+++[->+<]  this is a comment, really.
>>,[.,]
<<< tail +
```

```eiffel
class APPLICATION create make
feature
    make
            -- Run it.
        local
            x: REAL_64
            done: BOOLEAN
        do
            x := 3.14 + .5 + 1.e2 + 0b101 + 0c17
            if x > 2 and then not done then
                print ("big%N")
            elseif x /= 0 or else x < -1 then
                io.put_string ('c')
            end
            across 1 |..| 10 as i loop print (i.item) end
        end
    ok?: BOOLEAN
    "unclosed
    still"
end
```

```turtle
PREFIX ex: <http://ex.org/>
ex:s ex:p ex:o , ex:o2 ;
     ex:q "esc \"q\"" ;
     ex:r 'single' ;
     ex:t true , 3.5e2 , -7 .
ex:list ex:has ( 1 2 3 ) .
<a> = <b> .
ex:x a <http://ex.org/T>, ex:Y .
```

```ntriples
<http://a/s> <http://a/p> "v"^^<http://www.w3.org/2001/XMLSchema#string> <http://a/g> .
_:x <http://a/p> _:y .
<http://a/s> <http://a/p> "multi word literal" .
<http://a/s#> <http://a/p> "x" .
```
