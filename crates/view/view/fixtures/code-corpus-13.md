```elm
module Main exposing (main)
import Html exposing (Html, div, text)
-- A comment
type Msg = Increment | Decrement
update : Msg -> Int -> Int
update msg model =
    case msg of
        Increment -> model + 1
        Decrement -> model - 1
main = text ("Count: " ++ String.fromInt 42 ++ 'x' |> identity)
{- block -}
```

```pascal
program Hello;
{ A comment }
var
  i: Integer;
  name: string = 'World';
begin
  for i := 1 to 10 do
    if (i mod 2 = 0) and (i > 2) then
      WriteLn('Even: ', i);
  (* another comment *)
  x := 3.14 * nil; // line comment
end.
```

```smalltalk
"A comment"
Object subclass: #Point
    instanceVariableNames: 'x y'
    classVariableNames: ''
    package: 'Demo'.
| a b |
a := 3 + 4.
b := a > 5 ifTrue: [ 'big' ] ifFalse: [ 'small' ].
Transcript show: b; cr.
#(1 2 3) do: [:each | Transcript show: each printString].
self halt. ^ true
```

```vhdl
-- A comment
library IEEE;
use IEEE.STD_LOGIC_1164.ALL;
entity counter is
  port ( clk : in std_logic; q : out integer range 0 to 15 );
end counter;
architecture rtl of counter is
  signal c : integer := 0;
begin
  process (clk) begin
    if rising_edge(clk) then c <= c + 1; end if;
  end process;
  q <= c when c < 16 else 0; x <= "1010"; y <= '1';
end rtl;
```

```sparql
# A comment
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
SELECT DISTINCT ?name (COUNT(?friend) AS ?count)
WHERE {
  ?person foaf:name ?name ;
          foaf:knows ?friend .
  FILTER (lang(?name) = "en" && ?count > 2)
  OPTIONAL { ?person foaf:age $age }
}
ORDER BY DESC(?count) LIMIT 10
```

```cypher
// A comment
MATCH (p:Person {name: 'Ada'})-[:KNOWS]->(friend)
WHERE friend.age > 30 AND NOT p = friend
RETURN friend.name AS name, count(*) AS total
ORDER BY total DESC LIMIT 5;
CREATE (n:City {name: "Paris", population: 2148000})
```

```d
// A comment
import std.stdio;
/* block */
/+ nested +/
@safe int square(int x) { return x * x; }
void main() {
    auto values = [1, 2.5, 0x1F];
    foreach (i, v; values) writeln(i, ": ", v);
    string s = `raw` ~ "text";
    if (s !is null && true) { return; }
}
```

```crystal
# A comment
require "json"
class Greeter
  property name : String
  def initialize(@name : String); end
  def greet(times = 2) : String
    "Hello, #{@name}!" * times
  end
end
greeter = Greeter.new("World")
puts greeter.greet if greeter.name.size > 0
values = [1, 2.5, 0x1F, :sym, 'c', %w(a b)]
```
