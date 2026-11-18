```rb
module Shop
  class Cart
    include Enumerable
    def initialize; @items = []; end
    def add(item, qty = 1) = @items << { item: item, qty: qty }
    def total
      @items.sum { |line| line[:item].price * line[:qty] }
    end
    def each(&block) = @items.each(&block)
  end
end
cart = Shop::Cart.new
cart.add(Product.new("Pen", 1.5), 3) unless cart.frozen?
puts "Total: %.2f" % cart.total
case value
when 1..5 then :low
when /\d+/ then :digits
else raise ArgumentError, "bad"
end
```

```php
<?php
declare(strict_types=1);
interface Shape { public function area(): float; }
final class Circle implements Shape {
    public function __construct(private float $r) {}
    public function area(): float { return M_PI * $this->r ** 2; }
}
$shapes = [new Circle(2), new Circle(3.5)];
usort($shapes, fn(Shape $a, Shape $b) => $a->area() <=> $b->area());
if (isset($_GET['id']) && is_numeric($_GET['id'])) {
    $id = (int) $_GET['id'];
} elseif (empty($id)) {
    throw new InvalidArgumentException('Missing id');
}
/* block
   comment */
echo json_encode(['ok' => true, 'count' => count($shapes)]);
```

```swift
enum Direction: String, CaseIterable {
    case north = "N", south = "S"
}
protocol Drawable { func draw() }
extension Point: Drawable {
    func draw() { print("(\(x), \(y))") }
}
var numbers = [3, 1, 2].sorted(by: >)
let squares = numbers.map { $0 * $0 }.filter { $0 > 1 }
for (index, value) in numbers.enumerated() where value % 2 == 0 {
    print(index, value)
}
func load() async throws -> Data {
    let (data, _) = try await URLSession.shared.data(from: url)
    return data
}
```

```lua
local Stack = {}
Stack.__index = Stack
function Stack.new() return setmetatable({ items = {}, size = 0 }, Stack) end
function Stack:push(v)
  self.size = self.size + 1
  self.items[self.size] = v
end
for k, v in pairs(config) do
  if type(v) == "table" then print(k, #v) end
end
local ok, err = pcall(function() error("boom") end)
while not ok do ok = true end
repeat x = x - 1 until x <= 0
```

```powershell
$users = Import-Csv -Path ".\users.csv"
foreach ($user in $users) {
    if ($user.Enabled -ne $false) {
        New-ADUser -Name $user.Name -Enabled $true
    } else {
        Write-Warning "Skipping $($user.Name)"
    }
}
$total = ($users | Measure-Object -Property Age -Sum).Sum
Get-Process | Sort-Object CPU -Descending | Select-Object -First 5
try { Invoke-RestMethod -Uri $url -Method Post } catch { Write-Error $_ }
<# block
comment #>
```

```r
library(dplyr)
scores <- c(90, 85.5, 77)
names(scores) <- c("a", "b", "c")
summary_df <- mtcars %>%
  filter(mpg > 20) %>%
  group_by(cyl) %>%
  summarise(avg = mean(hp), n = n())
square <- function(x) {
  stopifnot(is.numeric(x))
  return(x ^ 2)
}
while (i < 10) { i <- i + 1; next }
x[x == 0] <- NA_real_
`my var` <- 1e-3
```
