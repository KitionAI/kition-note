```elm
port module Ports exposing (..)
type alias Model = { count : Int, name : String }
view : Model -> Html Msg
view model =
  let
    label = "a\"b" ++ """multi
line""" ++ String.fromFloat 3.5e2
  in
  div [] [ text label, text (if model.count > 0x1F then "big" else "small") ]
{- outer {- inner -} still -}
_ = \x -> x
```

```pascal
unit Shapes;
interface
type
  TShape = class(TObject)
  private
    FArea: Double;
  public
    function Area: Double; virtual; abstract;
  end;
implementation
const Hex = $FF; Chars = #13#10;
procedure Draw(var S: TShape);
begin
  repeat Inc(i) until i >= 10;
  S := nil; if not Assigned(S) then Exit;
end;
end.
```

```smalltalk
| count total |
count := 0.
total := #(3 4 5) inject: 0 into: [:sum :each | sum + each].
$a isVowel ifTrue: [count := count + 1].
#symbol printNl. #with:with: value. #'quoted symbol' size.
selfish := thisContext sender.
2r1010 printString: 16.
^ super new init
```

```vhdl
LIBRARY ieee;
ENTITY mux IS
  GENERIC ( WIDTH : natural := 8 );
  PORT ( a, b : IN std_logic_vector(WIDTH-1 DOWNTO 0); sel : IN bit; y : OUT std_logic_vector(WIDTH-1 DOWNTO 0) );
END ENTITY;
ARCHITECTURE behave OF mux IS
BEGIN
  y <= a WHEN sel = '0' ELSE b;
  gen: FOR i IN 0 TO 3 GENERATE
    assert x /= 16#FF# report "bad" severity warning;
  END GENERATE;
END ARCHITECTURE;
```

```sparql
BASE <http://example.org/>
PREFIX ex: <http://example.org/ns#>
CONSTRUCT { ?s ex:label ?l }
WHERE {
  ?s a ex:Thing ; ex:name ?l .
  BIND(STRLEN(?l) AS ?len)
  FILTER(?len >= 3 || regex(?l, "^A", "i"))
  VALUES ?type { ex:A ex:B }
}
GROUP BY ?s HAVING (COUNT(*) > 1)
```

```cypher
MERGE (a:Account {id: $id})
ON CREATE SET a.created = timestamp()
WITH a
MATCH p = shortestPath((a)-[*..5]-(b:Account))
WHERE b.balance >= 100 AND a <> b
UNWIND nodes(p) AS n
RETURN DISTINCT n.id, size(collect(n)) AS c
SKIP 0 LIMIT 10
```

```d
module app.main;
import std.algorithm : map, filter;
enum Color { red, green }
struct Point { double x = 0.5; double y; }
class Shape { abstract double area() const; }
T max(T)(T a, T b) pure nothrow @nogc { return a > b ? a : b; }
unittest { assert(max(1, 2) == 2); }
auto r = [1, 2, 3].map!(x => x * 2).filter!(x => x > 2);
char c = 'x'; wstring w = "wide"w;
/+ nested /+ inner +/ outer +/
```

```crystal
module Shapes
  abstract class Shape
    abstract def area : Float64
  end
  struct Point
    getter x : Int32, y : Int32
    def initialize(@x, @y); end
    def +(other : Point) : Point
      Point.new(x + other.x, y + other.y)
    end
  end
end
macro define(name)
  def {{name.id}}; end
end
case value
when Int32 then puts "int #{value * 2}"
else puts %(other)
end
items.each_with_index { |item, i| p item, i }
h = {"a" => 1, :b => 2_000}
```
