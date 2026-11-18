```yacas
// Yacas session
f(x) := x^2 + 3*x;
Deriv(x) f(x);
Integrate(x, 0, Pi) Sin(x);
a_b := {1, 2.5, .5e3};
10 # fac(n_IsInteger) <-- n * fac(n - 1);
s := "hello \"world\"";
/* block
comment */ Simplify(a+b)
'quoted $x
```

```mumps
HELLO ; A routine
 WRITE "Hello, World!",!
 SET X=1,Y=$PIECE("a^b","^",2)
 IF X'=2 DO SUB
 FOR I=1:1:10 WRITE I," "
 QUIT
SUB(A) ; subroutine
 NEW %T S %T=$H
 K ^GLOBAL(1) W:X>0 "pos"
 Q 1.5E3
```

```pig
-- Load and filter
raw = LOAD 'data.txt' USING PigStorage(',') AS (name:chararray, age:int);
adults = FILTER raw BY age >= 18;
grouped = GROUP adults BY name;
counts = FOREACH grouped GENERATE group, COUNT(adults) AS n;
/* store it */
STORE counts INTO 'out' USING PigStorage();
x = foreach y generate flatten($0), 3.5, "dq";
```

```fcl
FUNCTION_BLOCK tipper
VAR_INPUT
    service : REAL;
END_VAR
FUZZIFY service
    TERM poor := (0, 1) (4, 0) ;
    TERM good := (1, 0) (4,1) (6,1) (9,0);
END_FUZZIFY
DEFUZZIFY tip
    METHOD : COG;
    DEFAULT := 0x1F + 0.5 + .25e2 + 017;
END_DEFUZZIFY
RULEBLOCK No1
    RULE 1 : IF service IS poor THEN tip IS cheap; // comment
    (* block *) /* other */
END_RULEBLOCK
END_FUNCTION_BLOCK
```

```oz
declare
fun {Fact N}
   if N == 0 then 1 else N * {Fact N-1} end
end
local X Y in
   X = 5 Y = ~3.5
   {Browse X + Y}
end
class Counter
   attr val
   meth init val := 0 end
   meth inc(V) val := @val + V end
end
proc {Show X} {Browse X} end
L = [1 2 3] % a list
S = "string" A = 'atom' B = true C = nil
/* block */ D = 0xFF E = 1.5e~3 F = [] G :: 3
```

```idl
; IDL program
pro hello, name
  compile_opt idl2
  print, 'Hello, ', name
  x = findgen(100) * 2.5d0 + 0x1F
  if n_elements(x) gt 10 then begin
    plot, x, sin(x), title="Sine ""wave"""
  endif
  foo = myfunc(x) & bar = -1.5e-3
end
```

```webidl
// Web IDL
[Exposed=Window, Constructor(DOMString name)]
interface Person : Base {
  readonly attribute DOMString name;
  attribute unsigned long age;
  const short LIMIT = 0x10;
  Promise<void> greet(optional DOMString msg = "hi");
};
dictionary Options { boolean flag = true; double ratio = 1.5e3; };
typedef sequence<long> LongList;
enum Color { "red", "green" };
Person implements Mixin; /* block
comment */ callback Cb = void (any value);
```

```mirc
; mIRC script
alias greet {
  var %name = $1
  if (%name == $null) { echo -a No name | return }
  msg $chan Hello, %name $+ !
  /* block
  comment */ echo $calc(1 + 2) \n
}
on *:TEXT:!hi*:#:{ notice $nick "hey" }
```
