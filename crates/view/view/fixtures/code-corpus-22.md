```yacas
_x + x_ + a_b_c + x'' ;
While(i < 10) [ i++; ];
Taylor(x, 0, 3) Exp(x) @ y ~ z # w
2.  3.14E+2  0.5e-1  12 #
"open
string"
f := {a, b} . [c]; `q
```

```mumps
 . WRITE "nested"
 D ^ROUTINE S A(1,2)=3
 W $L("abc"),$EXTRACT(S,1,2),$$FUNC^RTN(X)
 I A]]B!(C[D) Q
 S X=@Y,Z=-5,W=+3.2
LABEL2 Q:X'>0  W "x"
 set lower=1 write lower
	TAB W "tab line"
```

```pig
A = LOAD 'x' AS (f1:int, f2:bag{t:tuple(a:long)});
B = ORDER A BY f1 DESC PARALLEL 4;
C = DISTINCT B; D = LIMIT C 10;
E = JOIN A BY f1 LEFT OUTER, B BY f1;
F = FOREACH E GENERATE A::f1 AS x, (f2 IS NULL ? 0 : 1);
DUMP F; store F into 'p' using JsonStorage();
G = filter F by x matches '.*\'q.*';
H = 'unterminated
```

```fcl
VAR_OUTPUT tip : REAL; END_VAR
ACCU : MAX;   ACT : MIN;
RULE 2 : IF service IS good OR food IS nice THEN tip IS average WITH 0.5;
TERM x := 1.e3 - 2E-1 + 0.0;
x := NaN; y := true AND false; (*multi
line*) z := cogs;
```

```oz
functor
import Application System
define
   proc {Loop N} if N > 0 then {System.showInfo N} {Loop N-1} end end
   case X of nil then skip [] H|T then {Loop H} end
   R = {List.map [1 2] fun {$ I} I*2 end}
   class `Weird Name` end
   meth `odd`(X) skip end
   thread {Delay 100} end
   try raise err end catch E then skip finally skip end
   X <- 1 Y := 2 Z =< 3 W \= 4 V :: 5 U ... T =<: 6
   N = ~0x1F M = 007 Q = 2.5e+3 'atom' "str\"ing"
end
```

```idl
function area, r
  compile_opt idl2, hidden
  return, !pi * r^2  ; comment
end
for i=0L, n-1 do begin & print, i & endfor
a = [1.5e2, .5, 3.D-4, -2, +7] # b ## c
str = 'it''s' + "dq" + 'unclosed
while x ne 0 do x = x / 2
case op of 'add': y = a + b & else: y = 0 & endcase
```

```webidl
[NoInterfaceObject] partial interface Window {
  getter any (unsigned long index);
  setter void (DOMString name, any value);
  stringifier;
  iterable<DOMString, long>;
  attribute (DOMString or long)? union;
  long long big = -0x7F, small = 08, f = -.5, g = 1e10;
};
typedef unsigned long long Size;
callback interface Handler { void handle(); };
interface Child : Person { };
dictionary Opt : Base { required DOMString id; };
A implements B;
```

```mirc
on *:JOIN:#test:{
  if ($nick == $me) { halt }
  elseif ($nick isop #) { mode # +v $nick }
  else { set %count $calc(%count + 1) | echo 4 -a %count joins }
  var %x = $replace($1-, a, b) ; trailing comment
  ;( ( meta block )) ;
  while (%i <= 10) { inc %i }
}
alias -l x { return $iif($1 > 2, big, small) }
```
