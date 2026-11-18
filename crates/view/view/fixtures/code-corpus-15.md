```verilog
`timescale 1ns / 1ps
`define WIDTH 8
module counter #(parameter N = 4) (
  input wire clk, rst_n,
  output reg [N-1:0] count
);
  // count up on each rising edge
  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) count <= 4'b0000;
    else count <= count + 1'b1;
  end
  /* a block
     comment */
  initial $display("count=%d \"q\"", count);
  assign y = 8'hFF & 3'o7 | 16'd42 ^ 2.5e-3;
  #10 x = 1_000;
endmodule
```

```systemverilog
class packet extends base;
  rand bit [7:0] data;
  function new(); super.new(); endfunction
  fork a(); b(); join_none
endclass
```

```tcl
# A procedure
proc greet {name {greeting "Hello"}} {
    set msg "$greeting, $name!"
    puts $msg
    return [string length $msg]
}
set items [list 1 2.5 three]
foreach item $items { if {$item eq "three"} { break } }
#* block comment *#
expr {$x + 10 * 2}
array set colors {red 1 green 2}
puts 'single'
```

```gherkin
@billing @smoke
Feature: Refund policy
  As a customer I want refunds

  Background:
    Given a store with "10" items

  Scenario Outline: Return an item
    Given I bought <count> items
    When I return "1" item # a note
    Then I have <left> items
    And the receipt says """yes"""
    But nothing else

    Examples:
      | count | left |
      | 5     | 4    |

  Scenario: Docstring
    Given the text:
      """
      some multi line
      text here
      """
    * a star step
```

```http
POST /api/items?limit=10 HTTP/1.1
Host: example.com
Content-Type: application/json
  folded header line

{"name": "widget"}
```

```http
HTTP/1.1 404 Not Found
Server: nginx
bad line without colon
```

```mathematica
(* compute a sum (* nested *) *)
f[x_] := x^2 + 3 x - 1
g[x_Integer, y_:0] := Module[{z = x + y}, z!]
data = Table[{i, Sin[i]}, {i, 0, 2 Pi, 0.1}];
Plot[f[x], {x, -2, 2}, PlotStyle -> Red]
f::usage = "f[x] squares x"
In[3] Out[2] #1 & /@ {1, 2, 3}
System`Pi 2^^1010 1.5*^3 \[Alpha] "str\"ing"
```

```vbscript
' A comment
Option Explicit
Dim fso, name, count
Set fso = CreateObject("Scripting.FileSystemObject")
name = "World" & " ""quoted"""
count = &HFF + 3.14 + 42 + .5
If count > 10 And Not IsEmpty(name) Then
  MsgBox "Hello " & name, vbOKOnly
ElseIf count = 0 Then
  WScript.Echo Err.Description
End If
For i = 1 To 10 : Next
Rem a remark
Function Add(ByVal a, ByRef b)
  Add = a + b
End Function
x = Nothing : y = True
```

```puppet
# Manage nginx
class nginx::server ($port = 80) inherits nginx {
  package { 'nginx':
    ensure => present,
  }
  file { "/etc/nginx/${name}.conf":
    ensure  => file,
    content => template('nginx/site.erb'),
    require => Package['nginx'],
  }
  service { 'nginx': ensure => running, enable => true }
  if $::osfamily == 'Debian' { include apt }
  $list = [1, 2, 3]
  notify { "port is $port": }
}
define site::vhost ($docroot) { }
node 'web01' { }
$x = /^web\d+$/
```
