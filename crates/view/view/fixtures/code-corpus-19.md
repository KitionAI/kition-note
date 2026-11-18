```mscgen
# A message sequence chart
msc {
  hscale = "1.5", arcgradient = 8;
  a [label="Client"], b [label="Server", linecolor="red"];
  a => b [label="request"];
  b >> a [label="reply \"ok\""];
  a note b [label="done"];
  --- [label="separator"];
  /* a block
     comment */
  a -> b, b box b;
}
```

```msgenny
a, b : "B";
a => b : request;
a alt b : maybe {
  b >> a : yes;
};
```

```xu
msc {
  watermark="draft";
  a, b;
  a loop b [label="retry"] { a => b; };
}
```

```ebnf
(* EBNF grammar *)
grammar = { rule } ;
rule = identifier , "=" , expression , ";" ;
expression = term , { "|" term } ;
term = factor , { "," factor } ;
factor = identifier | 'literal' | "[" expression "]" | [a-z] ;
identifier = letter , { letter | digit | "_" } ;
digit = "0" | "1" ; // line comment
call(x) . y
```

```spreadsheet
=SUM(A1:B10) + AVERAGE($C$1:C5) * 2
=IF(A1>0, "positive", 'neg')
=VLOOKUP(D2, Sheet2!A:B, 2, FALSE) / 3.5
12abc \x [range] {1;2}
```

```z80
; Print a string
start:  ld hl, msg
        ld b, (hl)
loop:   inc hl
        ld a, (hl)
        call print
        djnz loop
        jp nz, done
        ret
msg:    db "Hello", 0
        ld a, $FF
        ld b, %1010
        ld c, 'x'
        ld de, 0FFh
.label  push ix
#define FOO 1
```

```solr
title:"quick fox" AND body:jump*
+required -excluded ^boost
price:[10 TO 20] OR NOT stock:0
name:smith~2 age:42 rating:3.5
(a || b) && c
```

```troff
.TH LS 1 "2024" "Manual"
.SH NAME
ls \- list directory contents
.B bold text
.I italic
.\" a comment
\fBBold\fR and \fIitalic\fP \(em dash \*(Rq
.PP
plain line
.br
'\" another comment
```

```rpm-spec
# Spec file
Name:           hello
Version:        2.10
Release:        1%{?dist}
BuildArch:      noarch
%description
A program.
%prep
%setup -q
%build
%configure
make %{?_smp_mflags} $RPM_OPT_FLAGS ${HOME}
%if 0%{?fedora} >= 30
echo yes
%else
echo no
%endif
%changelog
```

```asciiarmor
-----BEGIN PGP PUBLIC KEY BLOCK-----
Version: GnuPG v2
Comment: test key

mQENBFzJ5aIBCAC7zZ3k/abc+def=
xyz123==
-----END PGP PUBLIC KEY BLOCK-----
trailing text
```

```pgp
-----BEGIN PGP MESSAGE-----
hQEMA1b2+3c=
-----END PGP SIGNATURE-----
```

```haskell-literate
This is a literate comment.
> module Main where
> main :: IO ()
> main = putStrLn "hi" -- comment
More prose here.
```

```hxml
# Build file
-cp src
-main Main
--interp
-D debug
-lib 'heaps'
-js bin/main.js
```
