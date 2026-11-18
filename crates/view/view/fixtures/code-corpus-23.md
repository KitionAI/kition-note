```livescript
# LiveScript sample
require! { fs, path }
square = (x) -> x * x
cube = -> it * it * it
class Animal extends Base
  (@name) ->
  speak: -> console.log "#{@name} speaks"
list = [1 2 3] |> map (* 2) |> filter (> 2)
obj = {a: 1, b-c: 2, 'quoted': 3}
if x is true and y isnt null then yes else no
for own key, value of obj then console.log key
nums = [0x1F, 2r1010, 1.5e3, 12_000, 36rZZ, .5]
re = /ab+c/gi ; here = //
  a b # comment
//g
str = 'single \'q\'' + "double #{x}"
doc = '''multi
line'''
words = <[ alpha beta gamma ]>
js = `alert(1)`
\word + \another
try throw new Error 'x' catch e then void
x?.y ... z ?? w
switch | a => b | otherwise => c
/* doc
comment */ fun = !-> it
```
