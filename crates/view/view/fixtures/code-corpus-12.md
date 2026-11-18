```vb
#Region "Helpers"
Public Class Counter
    Private _value As Long = &HFF
    Public Function Add(ByVal x As Integer) As Integer
        _value += x
        Return CInt(_value) * 2 Mod 3
    End Function
    Dim s As String = "say ""hi"""
    Dim f = 1.5F + .25 + 3.0 + 7L
    Do While x <= 10 Or Not done
        x = x \ 2 ^ 1
    Loop
End Class
#End Region
```

```ocaml
(* nested (* comment *) here *)
let greet ~name ?(greeting = "Hi") () = greeting ^ ", " ^ name
let x = 1_000_000 + 0b101 + 0o17 + 1.5e3
let rec map f = function [] -> [] | h :: t -> f h :: map f t
let s = {|long
string|}
exception Not_found_here of string
let _ = match x with Some v when v > 0 -> v | _ -> raise Not_found
class point = object val mutable x = 0 method get = x end
```

```fsharp
namespace Demo
[<EntryPoint>]
let main argv =
    let xs = seq { for i in 1 .. 10 do yield i * i }
    let result = xs |> Seq.filter (fun x -> x % 2 = 0) |> Seq.sum
    printfn "Result: %d" result
    if result > 100 then failwith "big" else ()
    0 // exit code
type Shape = | Circle of radius: float | Square of side: float
```

```scheme
#;(ignored form) (define x 1)
(define-syntax swap!
  (syntax-rules ()
    ((_ a b) (let ((tmp a)) (set! a b) (set! b tmp)))))
(list 1/2 -3 +5 .5 1e10 #x1F #b101 #e1.5 1+2i)
`(a ,b ,@c)
(string-append "a\"b" |odd symbol|)
(vector-ref #(1 2 3) 0)
```

```lisp
(in-package :cl-user)
(defclass point () ((x :initarg :x :accessor point-x)))
(loop for i from 1 to 10 collect (* i i))
(multiple-value-bind (q r) (floor 7 2) (list q r))
(let* ((a 1) (b (+ a 1))) b)
#| block
   comment |#
(format nil "~{~a~^, ~}" '(1 2 3))
(char-code #\A) (eq 'foo '|Foo Bar|)
(+ 1.5e3 -2 3/4 #b101 #o17)
```

```fortran
module geometry
  use iso_c_binding
  implicit none
contains
  pure function area(r) result(a)
    real, intent(in) :: r
    real :: a
    a = 3.14159 * r**2
  end function area
end module geometry
! check: candidate .eqv. other
logical :: ok = .true.
call system_clock(count)
write(*, '(A, I0)') "n = ", 42
```

```coffeescript
fib = (n) ->
  if n < 2 then n else fib(n - 1) + fib(n - 2)
obj =
  name: 'x'
  nested:
    deep: true
 odd: 1
regex = /^\d+$/i
multi = ///
  ^ \d+ # digits
///
"string #{interp} end"
for own key, value of obj
  console.log key unless value is null
x = y ? z
@method?()
```
