```mscgen
msc {
  width = "600", wordwraparcs = on;
  a [label="A", textcolor="blue"], b, c [ID="x", ArcSkip="1"];
  a => b [label="call", arcskip="1"];
  a <<=>> b; a -x b; b :> c; a ... c; |||;
  c abox c [label="an \"abox\""], b rbox b;
  # hash comment
  a => b [label="unterminated
  still"];
  { a -> b; } X- Onwards ONE
}
```

```ebnf
letter = "A" | "B" (* inline *) | "C" ;
string = '"' , { character - '"' } , '"' ;
opt = [ "x" ] , ( group ) , { rep } ;
%token NUMBER %% %} $$ $1 <<name>> /keyword \x
assign := value ;
returns = return_value ;
char = [^\]abc] | "\"" | 'it\'s' ;
/* c style
comment */ tail
```

```spreadsheet
=SUMIF(Sheet1!$A$1:$A$10, ">5", B1:B10)
=CONCAT("a\"b", 'c''d') & 1.5E+3
=ROUND(-PI() * 2, 2) % TRUE
=[@Column] + {1,2;3,4} \\ \n
=A1 = "unterminated
next"
```

```z80
        org $8000
main:   ld a, 5         ; load
        add a, b
        ld (ix+2), a
        jr c, skip
        jp (hl)
        out (c), a
        ld hx, 1
        ld sp, 0FFFFh
        ld bc, 1010b
        ld d, 12d
        ld e, 'a'
        ld e, '\n'
skip:   ret z
        rst 38h
        ex af, af'
```

```solr
text:"escaped \" quote" AND -title:draft
year:[2000 TO *] boost^2.5
"unterminated
id:007 n:1.0 x:0.5 y:1e5 z:NaN q:Infinity
a&&b c|d !e title:(foo OR bar)
```

```troff
.TH TEST 7
.SS Sub section
.HP
.Bd -literal
.ab
.Ab
\m[blue]colored\m[] \s+2big\s-2 \(aq \u\d \%hy \&.
.R roman
'br
  .B indented
text with \fBbold\fP inside
```

```rpm-spec
Summary: A test(x)
License: MIT
%package devel
%files
%{_bindir}/hello
%ifarch x86_64 i686
%define debug 1
%endif
%if %{with_tests} && 0%{?rhel}
%{__make} check
%macro(1 2) arg
%post -p /sbin/ldconfig
%preun
ppc64le
```

```asciiarmor
before the armor
-----BEGIN PGP SIGNATURE-----

iQEzBAEBCAAdFiEE/x+y=
-----END PGP SIGNATURE-----
after
```

```haskell-literate
> import Data.List (sort)
>
> f :: [Int] -> Int
> f = sum . map (* 2)
  > not code
Prose with > inside.
```

```hxml
-cp src -main App
--next
--each
-D analyzer-optimize
-lib 'heaps
still string'
-dce full
```
