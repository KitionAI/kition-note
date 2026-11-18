```ruby
require 'json'
# A comment
class Greeter < Base
  attr_reader :name
  DEFAULT = "World".freeze
  def initialize(name = DEFAULT)
    @name = name
    @@count ||= 0
  end
  def greet(times: 1)
    times.times { |i| puts "Hello, #{@name} #{i}!" }
    return nil if $stdout.nil?
    [1, 2.5, 0x1f].map(&:to_s)
  end
end
items.each do |item| next unless item =~ /ab+c/i end
Greeter.new('Ada').greet(times: 2) if true && !false
```

```php
<?php
namespace App\Models;
use Illuminate\Support\Str;
// comment
class User extends Model implements JsonSerializable {
    private array $items = [];
    public const MAX = 10;
    public function find(int $id, ?string $name = null): ?self {
        $result = $this->items[$id] ?? null;
        echo "User {$name} has $id items\n";
        return strlen($name) > 0 ? new static() : null;
    }
}
$list = array_map(fn($x) => $x * 2, [1, 2, 3]);
foreach ($list as $key => $value) { print($value); }
?>
```

```swift
import Foundation
// A comment
struct Point: Codable {
    var x: Double = 0
    let name: String
    func distance(to other: Point) -> Double {
        let dx = x - other.x
        return sqrt(dx * dx)
    }
}
@MainActor class ViewModel: ObservableObject {
    @Published var items: [String] = []
    private lazy var count = 42
}
let label = "Value: \(point.x) and \(1 + 2)"
guard let value = optional else { return }
if case .some(let v) = result, v > 0x1F { print(v, true, nil) }
```

```lua
-- A comment
local M = {}
function M.greet(name, ...)
  local count = #name
  for i = 1, 10 do
    print(string.format("Hello %s %d", name, i))
  end
  if count > 0 and not done then return true elseif x == nil then return false end
  local t = { key = "value", [1] = 2.5, nested = { 0xff } }
  return setmetatable(t, { __index = M })
end
--[[ block
comment ]]
local s = [[long
string]]
return M
```

```powershell
# A comment
$name = "World"
$items = @(1, 2, 3)
function Get-Greeting {
    param([string]$Name = "World", [int]$Times = 1)
    foreach ($i in 1..$Times) {
        Write-Output "Hello, $Name ($i)"
    }
}
Get-ChildItem -Path C:\Temp -Filter *.log | Where-Object { $_.Length -gt 1KB } | Remove-Item -WhatIf
if ($items.Count -eq 3 -and $true) { Write-Host 'three' -ForegroundColor Green }
```

```r
# A comment
library(ggplot2)
data <- read.csv("data.csv", header = TRUE)
mean_value <- function(x, na.rm = FALSE) {
  if (is.null(x)) return(NA)
  sum(x, na.rm = na.rm) / length(x)
}
result <- sapply(1:10, function(i) i ^ 2 + 0.5)
df$new <- ifelse(df$value > 3L, "high", 'low')
for (i in seq_along(result)) print(paste("Item", i, NULL, Inf))
```
