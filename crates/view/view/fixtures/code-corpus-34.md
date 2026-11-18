```nsis
!ifdef DEBUG
  !echo "debug build"
!endif
Var /GLOBAL count
StrCpy $count "0"
${ForEach} $R0 1 5 + 1
  DetailPrint "Item $R0"
${Next}
Page directory
UninstPage instfiles
SetCompressor /SOLID lzma
# hash comment
IfFileExists "$INSTDIR\app.exe" 0 +2
  Delete "$INSTDIR\app.exe"
'single' `back`
```

```wast
(module $m
  (type $t (func (param i64) (result f32)))
  (import "env" "log" (func $log (param i32)))
  (func (export "main") (local $i i32)
    (block $done (loop $top
      (br_if $done (i32.eqz (local.get $i)))
      (local.set $i (i32.sub (local.get $i) (i32.const -1)))
      (br $top)))
    (call $log (i32.const 0x2A)) nan inf -0.5)
  (table 2 funcref) (elem (i32.const 0) $log)
  (; open
  comment ;)
)
```

```xquery
declare variable $limit as xs:integer := 10;
for $i at $pos in (1 to $limit)
let $s := fn:string($i)
where $i mod 2 eq 0
return
  <item n="{$i}" pos='{$pos}'>
    <name>{ $s }</name><!-- xml comment -->
  </item>
some $x in (1, 2) satisfies $x gt 1
"http://example.com":local, ancestor::div/@id
typeswitch ($n) case element(a) return "a" default return "z"
```

```cobol
000100 IDENTIFICATION DIVISION.
000200 PROGRAM-ID. CALC.
000300*COMMENT IN COLUMN SEVEN
       01 TOTAL PIC S9(5)V99 COMP-3 VALUE +123.45.
       01 FLAGS PIC X VALUE 'Y'.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 10
               DISPLAY "Line " I
           END-PERFORM.
           MOVE SPACES TO WS-NAME. MOVE 0x1F TO X.
           IF X = 'A' OR "B" THEN CONTINUE END-IF.        IDENTIFY
```

```sas
data _null_;
  file print;
  put 'Hello' _n_=;
  do i = 1 to 3; x = i ** 2; end;
  if name in ('a', 'b') and x ne 0 then output;
run;
proc sql;
  select name, age from sashelp.class where age ge 13;
quit;
%let x = 5;
%put &x;
libname mylib '/data';
footnote2 'note';
options nodate;
```

```ttcn
type component MyComp { port MyPort p; timer t; }
function f_send(in integer x) return boolean {
  var charstring s := "abc"B;
  var bitstring b := '1010'B;
  if (x > 0 and not false) { return true } else { return omit }
}
control { execute(tc_hello(), 5.0); }
template Msg t_msg := { id := ?, text := * } with { encode "XML" }
@lazy var integer y;
#include "x.ttcn"
%define MAX 10
```

```ttcn-cfg
[MAIN_CONTROLLER]
TCPPort := 9034
NumHCs := 1
[EXTERNAL_COMMANDS]
BeginControlPart := "begin.sh"
[GROUPS]
All := "host1", "host2"
"unterminated
```
