```stylus
@font-face
  font-family Foo
  src url("foo.woff") format('woff')
@media (min-width: 40em) and print
  table, pre
    border none
li:first-child, input[type="text"]
  color #fff
  background-color #ABCDEF80
.a
  @extend .b
  +button(red)
  {$prop} 10px
  height: calc(100% - 10px)
$theme = { color: red, size: 12px }
.c
  color $theme.color
  z-index 10 ** 2
sum(a, b)
  return a + b unless b is a 'unit'
.d
  width sum(1, 2) if $x isnt null
  a b, em col
* + *
  margin 0
@-webkit-keyframes spin
  from
    transform rotate(0deg)
  50%
    color Red
  to
    transform rotate(360deg)
@css {
  .raw { color: red; }
}
~sibling
  &-suffix
    -moz-box-sizing border-box
    opacity .5 !optional
"multi \
line"
x = y ? 1 : 2
```
