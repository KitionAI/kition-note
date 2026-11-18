```vb
' A comment
Imports System.Collections.Generic
Module Program
    Sub Main(args As String())
        Dim count As Integer = 42
        Dim ratio As Double = 2.5
        Dim name = "World"
        If count > 10 AndAlso name <> "" Then
            Console.WriteLine("Hello, " & name)
        End If
        For i = 1 To 10 Step 2 : Next
    End Sub
End Module
```

```ocaml
(* A comment *)
let rec fact n = if n <= 1 then 1 else n * fact (n - 1)
type shape = Circle of float | Rect of float * float
let area = function
  | Circle r -> 3.14 *. r *. r
  | Rect (w, h) -> w *. h
let () = print_endline (string_of_int (fact 5))
module M = struct let x = 0x1F end
let s = "text" ^ String.make 2 'c'
```

```fsharp
// A comment
open System
type Point = { X: float; Y: float }
let distance p q = sqrt ((p.X - q.X) ** 2.0 + (p.Y - q.Y) ** 2.0)
let numbers = [1; 2; 3] |> List.map (fun x -> x * 2)
match numbers with
| [] -> printfn "empty"
| head :: _ -> printfn "%d" head
let mutable total = 0
```

```scheme
; A comment
(define (square x) (* x x))
(define pi 3.14159)
(let ((a 1) (b "two"))
  (display (+ a (square 4)))
  (newline))
(if (null? '()) #t #f)
(map (lambda (n) (* n 2)) '(1 2 3))
#| block
   comment |#
```

```lisp
;; A comment
(defun greet (name &optional (greeting "Hello"))
  (format t "~a, ~a!~%" greeting name))
(defparameter *count* 42)
(let ((x 1.5) (y #x1F))
  (setq x (+ x y)))
(mapcar #'(lambda (n) (* n 2)) '(1 2 3))
(defmacro unless* (test &body body) `(if ,test nil (progn ,@body)))
(print :keyword) (print nil)
```

```fortran
! A comment
program demo
  implicit none
  integer :: i, n = 10
  real(8) :: total = 0.0d0
  character(len=20) :: name = 'Fortran'
  do i = 1, n
    if (mod(i, 2) == 0 .and. i > 2) then
      total = total + sqrt(real(i))
    end if
  end do
  print *, "Total: ", total
end program demo
```

```coffeescript
# A comment
square = (x) -> x * x
class Animal
  constructor: (@name) ->
  speak: ->
    console.log "#{@name} makes a sound"
numbers = [1, 2.5, 0x1F]
evens = (n for n in numbers when n % 2 is 0)
if yes and not no then alert 'ok'
###
block comment
###
```
