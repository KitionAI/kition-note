```perl
package Counter;
our $VERSION = '1.02';
my $count = 0; # trailing comment
sub new { my ($class, %args) = @_; return bless { %args }, $class; }
my $text = qq{interpolated $count};
my @words = split /\s+/, $line;
unless (exists $hash{key}) { warn "missing\n"; }
while (my $row = <STDIN>) { chomp $row; last if $row eq 'q'; }
$total += $_ for @values;
my $ref = \@items;
print join(", ", map { $_ * 2 } grep { $_ > 1 } @$ref), "\n";
```

```haskell
{-# LANGUAGE OverloadedStrings #-}
module Geometry (Point(..), distance) where
import qualified Data.Map as M
newtype Wrapper a = Wrapper { unwrap :: a }
class Container f where
  empty :: f a
instance Show Point where
  show (Point x y) = "(" ++ show x ++ ", " ++ show y ++ ")"
distance :: Point -> Point -> Double
distance (Point x1 y1) (Point x2 y2) = sqrt ((x2 - x1) ^ 2 + (y2 - y1) ^ 2)
safeDiv _ 0 = Nothing
safeDiv a b = Just (a `div` b)
evens = [x | x <- [1..10], even x, x /= 4]
main = case lookup 'a' table of
  Just v -> print v
  Nothing -> return ()
```

```groovy
package demo
/* block
   comment */
@Grab('org.apache:commons:1.0')
interface Greeter { String greet() }
enum Color { RED, GREEN }
def list = [1, 2, 3].collect { it * 2 }
def text = """multi
line ${list.size()} string"""
assert list.size() == 3 : 'size'
for (int i = 0; i < 10; i++) { println "i=$i" }
def closure = { String a, b -> a + b }
static void main(String[] args) { new Person().with { name = 'x' } }
```

```nginx
worker_processes auto;
events { worker_connections 1024; }
http {
    include mime.types;
    gzip on;
    upstream backend { server 10.0.0.1:8080 weight=5; }
    server {
        listen 443 ssl;
        ssl_certificate "/etc/ssl/cert.pem";
        location = /health { return 200 'ok'; }
        if ($request_method = POST) { return 405; }
        rewrite ^/old/(.*)$ /new/$1 permanent;
        add_header X-Frame-Options DENY always;
    }
}
```

```cmake
cmake_minimum_required (VERSION 3.20)
option(BUILD_TESTS "Build the tests" ON)
find_package(Threads REQUIRED)
set(SOURCES
  src/a.cpp
  src/b.cpp)
target_include_directories(app PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include)
foreach(item IN LISTS SOURCES)
  message("Item: ${item} and $ENV{HOME}")
endforeach()
if(NOT DEFINED VERSION AND MSVC)
  add_compile_options(/W4 -Wall)
endif()
```

```protobuf
syntax = "proto2";
import "google/protobuf/timestamp.proto";
option java_package = "com.example.demo";
/* block comment */
message Order {
  required int64 id = 1 [default = 0];
  optional double total = 2;
  oneof payment { string card = 3; bool cash = 4; }
  reserved 5, 6 to 9;
  google.protobuf.Timestamp created = 10;
  message Item { uint32 qty = 1; bytes data = 2; fixed64 code = 3; }
}
service Orders { rpc Stream(stream Order) returns (stream Order) {} }
```
