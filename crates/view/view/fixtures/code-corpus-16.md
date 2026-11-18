```verilog
`include "defs.vh"
`uvm_object_utils_begin(packet)
`MY_MACRO (a, b)
module top; wire [3:0] a = 4'sd5, b = 'hx?, c = 'b1z0_1;
  real r = 1.5e3 + 2.0 + 3E-2 + 1_0.0_1;
  always @* x = $random % 8 ? y : z;
  always @(*) begin : named_block
    $finish; $ ; x <= ~&y || !z;
    @@ev; #1.5 q = 'd 12;
  end
  initial s = "line one \
line two";
  logic [7:0] mem [0:255]; /* open
  still */ assign q = a >>> 2;
endmodule // done
```

```tcl
set arr(key) "value"
puts $arr("key")
if {$x > 1} { puts "big" } elseif ($y == "s") { puts "s" }
#[[ a meta block ]]#
# [ [ spaced ]] # rest
#* multi
   line *# set y 2
SET Z 3; PUTS "q"
set p /"/ ; set n 0x1F
regsub -all {a+} $s "b" out
namespace eval ::app { variable count 0 }
$obj configure -text "Hi \"there\""
```

```gherkin
# language: en
Feature: Accounts
  Rule-free description with "quotes" and <angle>

  @wip
  Scenario Outline: Login as <role>
    Given a user "<name>" with role <role>
    And the password is "secret
    When they log in
    Then they see <page> # with comment

    Examples: Valid users
      | name  | role  | page      |
      | alice | admin | dashboard |
      | bob   | guest | home      |

  Scenario: Table step
    Given these items:
      | sku | qty |
      | a1  | 2   |
    Then done "x" <notplaceholder>
```

```http
GET /index.html HTTP/2.0
Accept: */*
Cookie: a=1; b=2
```

```http
HTTP/1.1 200 OK
Content-Length: 12
```

```http
HTTP/1.0 301 Moved
```

```http
HTTP/1.1 999 Weird
```

```http
get /lower
```

```http
DELETE /items/1 HTTP/1.1 extra
```

```mathematica
rule = x :> y + a:b + p:_Integer
name: anything goes here
opt -> 1.5`20 + 16^^FF.8 + 2.`` + 3``4 + 7*^-2
Out[] ##2 #name &
f[x_?NumericQ, y__, z___] := x /; x > 0
Global`x` + Context`Sub`f[1]
a::b: c::d \[Alpha]\[Beta] 'q' ~ x
s = "unclosed
still string" (* open
(* nested *) still *) done
{1, 2} . {3, 4} // N
```

```vbscript
Response.Write "x" : obj.Count = 3 : obj. Bad
x = &H1F + &O17 + 1.5F + 0 + 00 + 1e5 + 12L
s = "unclosed
On Error Resume Next
Select Case x
  Case 1: y = "one"
  Case Else: y = Empty
End Select
Do While i < 10 : i = i + 1 : Loop
Private Sub Foo() : Call Bar(1, 2) : End Sub
z = Replace(UCase(s), "A", "B") Mod 3 Xor 1 Is Nothing
WScript.Arguments.Item(0).Length
Class C : Public Property Get P : End Property : End Class
x = vbCrLf & vbcr & .5 & 3.
```

```puppet
@@nagios_host { $::fqdn: ensure => present }
@user { 'deploy': uid => 1001 }
Package { ensure => latest }
$msg = "Hello ${name}, \$notvar and $foo::bar::Baz done"
$long = "multi
line $tail"
case $::osfamily {
  'RedHat': { $pkg = 'httpd' }
  default: { fail("Unsupported ${::osfamily}") }
}
foo::bar { 'x': }
include foo::bar
File['/a'] -> Service['b'] ~> Exec['c']
$r = $x =~ /^a/ ? { true => 1, default => 2 }
$y = /unterminated
unless $z { notice($z) }
```
