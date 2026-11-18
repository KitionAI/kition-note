```tex
\newcommand{\R}{\mathbb{R}}
\begin{itemize}
  \item First \emph{item} with 50\% off
  \item[b)] Second, see \eqref{eq:1}
\end{itemize}
$$ \sum_{i=1}^{n} i = \frac{n(n+1)}{2} $$
\( a \leq b \) and \$5 \\
\begin{tabular}{|l|c|}
  1.5 & 2 \\ \hline
\end{tabular}
\importmodule[math]{sets}
```

```Dockerfile
FROM --platform=linux/amd64 python:3.12 as base
LABEL maintainer="dev@example.com" \
      version='2'
RUN apt-get update \
    && apt-get install -y curl # tools
COPY --from=base /src /dst
EXPOSE 80/tcp 443
HEALTHCHECK CMD curl -f http://localhost/ || exit 1
VOLUME ["/data"]
USER app
SHELL ["/bin/bash", "-c"]
ONBUILD
RUN echo "unterminated
```

```clojure
(defrecord Point [x y])
(defprotocol Shape (area [this]))
(comment
  (println "ignored") (+ 1 2))
(def ^:private secret 0x1F)
(def big 42N) (def dec 1.5M) (def sci 1e3)
(let [{:keys [a b]} {:a \a :b \newline}]
  (str/join ", " ['x "y" #"re+"]))
(->> (range 5) (map inc) (reduce +))
(user/foo 'sym @atom-ref)
#{1 2 3}
```

```erlang
-module(server).
-behaviour(gen_server).
-record(state, {count = 0 :: integer()}).
-define(TIMEOUT, 5000).
-spec handle_call(term(), pid(), #state{}) -> {reply, ok, #state{}}.
handle_call(Req, _From, State = #state{count = C}) when is_atom(Req) ->
    erlang:display(Req),
    Fun = fun(X) -> X * 2 end,
    receive
        {msg, Bin} when is_binary(Bin) -> <<Bin/binary, "!">>;
        Other -> ?TIMEOUT
    after 1000 -> 'quoted atom'
    end,
    Char = $a, Esc = $\n, Big = 2#1010, F = 1.5e-3,
    {reply, ok, State#state{count = C + 1}}.
```

```julia
#= block
   comment =#
@time result = map(x -> x^2, 1:10)
abstract type Animal end
mutable struct Dog <: Animal
    name::String
    tags::Vector{String}
end
f(x::Int)::Int = 2x + 1
const C = 0x1F + 0b101 + 1.5e-3 + 2im
s = raw"C:\path" * """triple
quoted""" * 'c' * '\n'
A = [1 2; 3 4]'
d = Dict(:a => 1, :b => 2)
isa(1, Int) && 1 in [1, 2]
for i in eachindex(A) println(i) end
```

```octave
%{
block comment
%}
x = [1, 2, 3]';
y = x.^2 + 3.5e-2 - 1i;
if x(1) >= 0 && ~isempty(y)
  printf("%d\n", numel(y));
elseif true
  z = {1, 'two'};
end
# hash comment
s = 'it''s';
f = @(t) sin(t) .* exp(-t);
while k < 10, k = k + 1; endwhile
result = A(2, :)' * B;
```
