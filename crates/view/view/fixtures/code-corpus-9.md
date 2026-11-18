```latex
\documentclass[11pt]{article}
\usepackage{amsmath, graphicx}
% A comment
\begin{document}
\section{Introduction}
Some text with \textbf{bold} and $x^2 + y_1$ inline.
\begin{equation}
  E = mc^2 \label{eq:energy}
\end{equation}
See Figure~\ref{fig:one} and \cite{knuth}.
\[ \int_0^1 f(x)\,dx \]
\end{document}
```

```dockerfile
# A comment
FROM node:20-alpine AS build
WORKDIR /app
COPY package*.json ./
RUN npm ci && npm run build
ENV NODE_ENV=production PORT=8080
EXPOSE 8080
ARG VERSION="1.0"
CMD ["node", "dist/index.js"]
ENTRYPOINT ["/bin/sh", "-c"]
```

```clojure
(ns demo.core
  (:require [clojure.string :as str]))
; A comment
(defn greet [name]
  (str "Hello, " name "!"))
(def numbers [1 2.5 3/4 -7])
(let [m {:a 1 :b "two"}]
  (println (get m :a) (count numbers)))
(map #(* % 2) (filter even? (range 10)))
(defmacro unless [test & body] `(if (not ~test) (do ~@body)))
```

```erlang
-module(demo).
-export([start/0, add/2]).
% A comment
add(A, B) -> A + B.
start() ->
    List = [1, 2.5, 16#ff],
    Map = #{name => "Ada", age => 36},
    case lists:sum(List) of
        N when N > 10 -> io:format("big ~p~n", [N]);
        _ -> {ok, Map}
    end.
```

```julia
module Demo
# A comment
using LinearAlgebra
function area(r::Float64)::Float64
    return π * r^2
end
xs = [1, 2.5, 3e2]
squares = [x^2 for x in xs if x > 1]
struct Point{T}
    x::T
end
println("total: $(sum(xs)) and $xs")
end
```

```octave
% A comment
function y = square(x)
  y = x .^ 2;
end
A = [1 2; 3 4];
b = A' * [5; 6];
for k = 1:10
  if mod(k, 2) == 0, disp(k); end
end
s = 'single' ; t = "double";
plot(linspace(0, 1, 100), sin(2*pi*(0:99)/100));
```
