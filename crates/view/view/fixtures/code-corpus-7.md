```perl
#!/usr/bin/perl
use strict;
use warnings;
# A comment
my $name = "World";
my @items = (1, 2.5, 0x1f);
my %ages = (alice => 30, 'bob' => 25);
sub greet {
    my ($who, $times) = @_;
    print "Hello, $who!\n" for 1 .. $times;
    return scalar @items;
}
if ($name =~ /^W\w+/i && defined $ages{alice}) { greet($name, 2); }
foreach my $key (sort keys %ages) { printf("%s: %d\n", $key, $ages{$key}); }
```

```haskell
module Main where
import Data.List (sortBy)
-- A comment
data Shape = Circle Double | Rect Double Double deriving (Show, Eq)
area :: Shape -> Double
area (Circle r) = pi * r ^ 2
area (Rect w h) = w * h
main :: IO ()
main = do
  let shapes = [Circle 1.5, Rect 2 3]
  mapM_ (print . area) shapes
  putStrLn $ "Total: " ++ show (sum (map area shapes))
{- block
   comment -}
```

```groovy
import groovy.json.JsonSlurper
// A comment
class Person {
    String name
    int age = 30
    def greet(String other) { return "Hello ${other}, I am $name" }
}
def people = [new Person(name: 'Ada'), new Person(name: "Bob", age: 41)]
people.findAll { it.age > 35 }.each { println it.name }
def map = [a: 1, b: 2.5, c: true, d: null]
```

```nginx
# A comment
server {
    listen 80 default_server;
    server_name example.com www.example.com;
    root /var/www/html;
    location / {
        try_files $uri $uri/ =404;
    }
    location ~ \.php$ {
        proxy_pass http://127.0.0.1:9000;
        proxy_set_header Host $host;
    }
}
```

```cmake
cmake_minimum_required(VERSION 3.16)
project(Demo LANGUAGES CXX)
# A comment
set(CMAKE_CXX_STANDARD 17)
add_executable(demo src/main.cpp)
if(WIN32)
  target_link_libraries(demo PRIVATE ws2_32)
endif()
message(STATUS "Building ${PROJECT_NAME} version ${PROJECT_VERSION}")
```

```protobuf
syntax = "proto3";
package demo.v1;
// A comment
message User {
  string name = 1;
  int32 age = 2;
  repeated string tags = 3;
  map<string, int64> scores = 4;
  enum Role { USER = 0; ADMIN = 1; }
}
service Users { rpc Get(User) returns (User); }
```
