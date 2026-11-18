```nsis
; NSIS installer
!define APP "MyApp"
!include "MUI2.nsh"
Name "${APP}"
OutFile "setup.exe"
InstallDir "$PROGRAMFILES\MyApp"
RequestExecutionLevel admin
Section "Main" SEC01
  SetOutPath $INSTDIR
  File /r "dist\*.*"
  WriteRegStr HKLM "Software\MyApp" "Path" $INSTDIR
  ${If} $0 == 1
    MessageBox MB_OK "Done $(^Name)"
  ${EndIf}
  IntOp $1 $1 + 0x10
SectionEnd
/* block
comment */ Function .onInit
FunctionEnd
```

```wast
(module
  ;; a comment
  (func $add (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add)
  (memory 1)
  (data (i32.const 0) "hello\n")
  (export "add" (func $add))
  (; block comment ;) (global $g (mut f64) (f64.const 1.5e3))
)
```

```xquery
xquery version "3.1";
declare namespace html = "http://www.w3.org/1999/xhtml";
declare function local:greet($name as xs:string) as xs:string {
  concat("Hello, ", $name)
};
(: a comment (: nested :) :)
for $b in doc("books.xml")//book
where $b/price > 30 and $b/@year = "2000"
order by $b/title
return <li class="book">{data($b/title)} text</li>
let $x := 1 + 2.5E2 return count(child::item)
element div { attribute id { "x" } }
```

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HELLO.
      * A comment line
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-COUNT PIC 9(3) VALUE ZERO.
       01 WS-NAME  PIC X(20) VALUE "World".
       PROCEDURE DIVISION.
           DISPLAY 'Hello, ' WS-NAME.
           ADD 1 TO WS-COUNT.
           IF WS-COUNT > 10 THEN MOVE 0 TO WS-COUNT END-IF.
           COMPUTE X = Y * 2.5 + -3.
           STOP RUN.
```

```sas
/* SAS program */
data work.class;
  set sashelp.class;
  if age > 12 then teen = 1;
  else teen = 0;
  bmi = weight / (height ** 2) * 703;
run;
proc means data=work.class mean max;
  var height weight;
run;
%macro hello(name);
  %put Hello &name;
%mend;
* a statement comment;
title1 "Report";
```

```ttcn
module Example {
  import from Types all;
  type record Msg { integer id, charstring text optional }
  testcase tc_hello() runs on MyComp system SysComp {
    var integer x := 0x1F;
    timer t := 5.0;
    t.start;
    p.send(Msg:{ id := 1, text := "hi" });
    alt { [] p.receive { setverdict(pass); } [] t.timeout { setverdict(fail); } }
    // comment
    /* block */ if (x > 2) { log("x"); }
  }
}
```

```ttcn-cfg
[LOGGING]
LogFile := "logs/%e.%h-%r.%s"
FileMask := LOG_ALL | DEBUG
ConsoleMask := ERROR | WARNING
# comment
[EXECUTE]
Example.tc_hello
[MODULE_PARAMETERS]
tsp_port := 8080
```
