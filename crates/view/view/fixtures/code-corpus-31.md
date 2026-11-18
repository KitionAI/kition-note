```sml
(* Standard ML *)
fun fact 0 = 1
  | fact n = n * fact (n - 1)
val x : int = 42 andalso true
datatype shape = Circle of real | Square of real
structure S = struct val s = "str\"ing" end // not a comment?
case x of 0 => "zero" | _ => "many"
```

```mllike
let rec f x = if x then 1 else 0 in f true
type t = { a : int }
match x with Some y -> y
```

```squirrel
// Squirrel script
local x = 10;
function add(a, b) { return a + b; }
class Foo extends Bar {
  constructor() { base.constructor(); }
  static count = 0;
}
foreach (i, v in arr) print(v + "\n");
#define NOPE 1
local t = typeof x == "integer" ? true : null;
```

```objective-c++
#import <Foundation/Foundation.h>
@interface Greeter : NSObject
- (void)greet:(NSString *)name;
@end
namespace app { class Thing { public: int n = 0; }; }
@implementation Greeter
- (void)greet:(NSString *)name { std::cout << "hi" << std::endl; NSLog(@"Hi %@", name); }
@end
```

```haxe
package app;
import haxe.ds.StringMap;
class Main extends Base implements IThing {
  static var count:Int = 0;
  public function new(name:String) { super(); this.name = name; }
  static function main() {
    var x = 5, y:Float = 1.5e3;
    for (i in 0...10) trace(i);
    var obj = { a: 1, b: "two" };
    #if js
    js.Browser.alert("hi");
    #end
    @:keep var re = ~/ab+c/gi;
    if (x > 2) return null; else throw "err";
  }
}
```

```modelica
model Pendulum "A simple pendulum"
  parameter Real L = 1.0 "length";
  Real theta(start = 0.1), omega;
equation
  der(theta) = omega;
  der(omega) = -9.81 / L * sin(theta); // gravity
  /* block */ 'quoted name' := 2e-3 .+ x;
end Pendulum;
```

```ecl
#OPTION('obfuscateOutput', TRUE);
Layout := RECORD
  STRING20 name;
  UNSIGNED4 age;
END;
ds := DATASET('~file', Layout, THOR);
OUTPUT(SORT(ds, -age), NAMED('sorted')) : PERSIST('p');
x := IF(COUNT(ds) > 10, 'many', "few"); // comment
/* block */ y := 0x1F + 3.5;
```

```dylan
Module: hello
define method say-hello (name :: <string>) => ()
  format-out("Hello, %s!\n", name);
end method say-hello;
define constant $pi = 3.14159;
define variable *count* = 0;
let x = #t; let k = #key; let h = #xFF;
if (x ~= 2) signal("bad") else block () end end;
with-open-file (s = "f") end; // comment
/* nested /* comment */ still */ x := foo: 1;
```
