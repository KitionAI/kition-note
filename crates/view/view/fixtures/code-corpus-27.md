```stylus
// Variables
font-size = 14px
$primary = #3fa9f5
main-color ?= red

body
  font 12px Helvetica, Arial, sans-serif
  color $primary
  margin 0 auto !important

a.button
  &:hover
    background lighten($primary, 10%)
  .icon
    width 16px

border-radius(n)
  -webkit-border-radius n
  border-radius n

/* block
   comment */
@media screen and (max-width: 600px)
  .nav
    display none

@keyframes pulse
  0%
    opacity 1
  100%
    opacity 0.5

for i in 1..3
  .col-{i}
    width (i * 10)%

if $primary is defined
  .ok
    color green
@import "base.styl"
ul li > a, #main
  border 1px solid rgba(0, 0, 0, .2)
  background url(images/bg.png)
```
