---
title: Introduction to the Untyped Lambda Calculus (β-reduction and α-conversion)
pubDate: 2025-11-04
---

Computability theory is a field that originated to understand what mathematical problems can be solved by a deterministic procedure, or, in other words, what problems can be solved by a deterministic algorithm. It started to be a thing because Alan Turing discovered the existence of unsolvable mathematical problems, and by unsolvable, I mean algorithmically unsolvable or not solvable by a Turing program. To properly reason and interact with these problems and due to the need to create some method to model computation, some guys created models of computation, like Turing machines, general recursive functions, and lambda calculus. Alonzo Church, introduced lambda calculus, in 1930s, because he was investigating the foundations of mathematics, and later to reason about what can be computed. 

Given that context, now it is easier to understand what it means to say that Lambda Calculus is a semantic for computation, which is quite useful for formally studying properties of computation. Or, even shorter, that Lambda Calculus is a model of computation. 

The semantics defined by the LC (Lambda Calculus) is that functions doesn't have a name, and that functions only uses functions of a single input. It means that a function that takes two inputs, must be rewritten by currying, in order to transform that function into a chain of functions that each takes a single argument: 
```rust
(x, y) -> x + y 
// ((x, y) -> x + y)(1, 2)
// 1 + 2 = 3

```
is rewritten into:
```rust
x -> (y -> x + y) 
// (x -> (y -> x + y))(1)(2)
// (y -> 1 + y)(2)
// 1 + 2 = 3
```

As you noticed, functions are also first class values, so a function can be passed as an argument and also be returned as output.

Computation is still about math, so computation models can be used to model mathematical problems. So, with the simple LC and its primitives (functions, variables and function applications) it is possible to model arithmetic, booleans, data structures and recursion and this technique is called Church encoding, at least nowadays. From what I know, Church didn't use this name on his book "The Calculi of Lambda-Conversion" and I don't know when people started to use it. Anyway, Church encoding let us represent pairs, booleans, optional values (e.g. none), natural numbers, lists... In fact, all functional data structures. 

Since I already mentioned his book, before showing arithmetic and numbers, here's himself defining positive integers:
<Image src="../../src/images/introlc/church-numeral-book.png" alt="Church's book defining positive integers with LC" />

As you can see, `1` is a function applied once, and two, is a function applied twice, and so on. Any natural number can be represented by applying a function `n` times to that number:
```rust
1 := (λf.λx. f x) // ((λf.λx. f x)y) => λx. y x
2 := (λf.λx. f (f x)) // ((λf.λx. f (f x))y)z => (λx. y (y x))z => y (y z)
```

In practice, for natural numbers, you can just count all `f` and you will know what number it is. But, you can also reduce the expression (a.k.a. computation) and check how many times `f` was applied. So, given an expression like `((λf.λx. f (f x))y)z` it is possible to define a substitution rule by following this rule: `(λx.M)N -> M[x:=N]`, where `M` is the body of the abstraction, and `N`, it's argument. Bringing it to our example: `(λx. f (f x))[f := y]` so, we replace all _free_ instances of `f` by `y`, and we have `(λx. y (y x))z` and then doing `(λx. y (y x))[x := z]` we have `y (y z)`, meaning the supplied function was applied twice.

Free is in italic because I didn't explain it yet. A variable is free when it is not bound by a surrounding lambda e.g. `λx. x` has `x` bound by `λx`. But `λx.y x` has `y` free because `x` is bound by `λx`. So `(λx. x y)[x := z]` reduces to `z y`, and `(λx. x y)[y := z]` to `λx. x z`.

A successor function is quite similar. Since two is `f` applied two times, to know it's successor, you should apply `f` one more time.

- succ := `λn.λf.λx.f (n f x)`, e.g. `(λn.λf.λx.f (n f x)) (λf.λx. f x)`. You should give it a try and reduce by yourself :)

I think the way I introduced LC helped to understand its primitives, so at this point you might have already noticed that LC is a programming language, and should also be able to understand that a variable is a valid lambda term, that an abstraction (or function) body is a valid lambda term, and also that an application (a function call) is a valid lambda term.

## Starting the interpreter

The LC AST is just a term, which means a variable, an abstraction, and an application. `Abs` tuple contains a `string` because it makes it easier to represent a Lambda symbol `λ`.


```ocaml
type term = Var of string | Abs of string * term | App of term * term
```

```ocaml
let rec to_string ~term =
  match term with
  | Var x -> x
  | Abs (x, body) -> "λ" ^ x ^ "." ^ to_string ~term:body
  | App (t1, t2) -> "(" ^ to_string ~term:t1 ^ " " ^ to_string ~term:t2 ^ ")"
```

A recursive helper calling itself for terms. It might be fun to follow the progress by printing expressions, so: 

```ocaml
let () =
  let term = App (Abs ("x", Var "x"), Abs ("y", Var "y")) in
  let result = to_string ~term in
  print_endline result
```

Output: `(λx.x λy.y)`

Seems good enough.

Now we need to compute/reduce it, so we must implement the same substitution rule we used to reduce it by hand: `(λx. M) N  →  M[x := N]` which is replacing all free occurrences of `x` with `N` in the body. So, it is just three variables, the term to substitute (the body), the variable to substitute, and the term to substitute with (the argument). So the signature for this function should look like this: `in_term:term -> variable:string -> by_term:term -> term`. It also should `reduce` over terms and call `subst` to substitute terms recursively. 


```ocaml
let rec subst ~in_term ~variable ~by_term =
  match in_term with
  | Var v -> if v = variable then by_term else Var v
  | App (t1, t2) ->
      App
        ( subst ~in_term:t1 ~variable ~by_term,
          subst ~in_term:t2 ~variable ~by_term )
  | Abs (x, t) ->
      if x = variable then Abs (x, t)
      else Abs (x, subst ~in_term:t ~variable:x ~by_term)
```

```ocaml
let rec reduce ~term =
  match term with
  | App (Abs (var, body), arg) -> subst ~in_term:body ~variable:var ~by_term:arg
  | App (lt, rt) ->
      let lt' = reduce ~term:lt in
      if lt <> lt' then App (lt', rt) else App (lt, reduce ~term:rt)
  | Abs (var, body) -> Abs (var, reduce ~term:body)
  | Var _ -> term
```
It is checking for the traditional case, like `(λx.x) y`. For those ones, we can just substitute `y` for `x` in the body of the abstraction. Too handle things like `((λx.x)(λy.y))z` we must loop over its terms. We're implementing a [normal-order](https://en.wikipedia.org/wiki/Lambda_calculus#Reduction_strategies) reduction, so it must first reduce the left term. It checks if `lt` and `lt'` differs, if so, it returns an application with the left-term reduced. Otherwise, the left-term is already reduced, so we must reduce the right one.

Here's the code until now:

```ocaml
type term = Var of string | Abs of string * term | App of term * term

let rec to_string ~term =
  match term with
  | Var x -> x
  | Abs (x, body) -> "λ" ^ x ^ "." ^ to_string ~term:body
  | App (t1, t2) -> "(" ^ to_string ~term:t1 ^ " " ^ to_string ~term:t2 ^ ")"

let rec subst ~in_term ~variable ~by_term =
  match in_term with
  | Var v -> if v = variable then by_term else Var v
  | App (t1, t2) ->
      App
        ( subst ~in_term:t1 ~variable ~by_term,
          subst ~in_term:t2 ~variable ~by_term )
  | Abs (x, t) ->
      if x = variable then Abs (x, t)
      else Abs (x, subst ~in_term:t ~variable:x ~by_term)

let rec reduce ~term =
  match term with
  | App (Abs (var, body), arg) -> subst ~in_term:body ~variable:var ~by_term:arg
  | App (lt, rt) ->
      let lt' = reduce ~term:lt in
      if lt <> lt' then App (lt', rt) else App (lt, reduce ~term:rt)
  | Abs (var, body) -> Abs (var, reduce ~term:body)
  | Var _ -> term

let () =
  let term = App (Abs ("x", Var "x"), Var "y") in
  let result = to_string ~term:(reduce ~term) in
  print_endline result
```

It prints to `y`, which is fine. But let's try to reduce this other expression: `(λx.λy.x)y` (`let term = App (Abs ("x", Abs ("y", Var "x")), Var "y")`). It prints `λy.x` because our interpreter doesn't know yet how to identify free variables, so we must implement alpha-conversion. 

To know if a term contains a free variable we must: 
- check if the variable is equal to the one we are looking for;
- if it is, then it is free;
- if it isn't, we check if it is an application or an abstraction; 
    - for an application, we check if it is free in the left or right term; 
    - for an abstraction, we check if the variable is equal to the one bound by the lambda; 
        - If it is, then it is not free; 
        - otherwise we must check if it is free in the body of the abstraction;

We can check if a variable is free, but we still need to rename it with an unique name.


```ocaml
let rec free_in ~variable ~term =
  match term with
  | Var x -> x = variable
  | App (lt, rt) -> free_in ~variable ~term:lt || free_in ~variable ~term:rt
  | Abs (x, body) ->
      if x = variable then false else free_in ~variable:x ~term:body
```


```ocaml
let fresh_var =
  let counter = ref 0 in
  fun base ->
    incr counter;
    base ^ string_of_int !counter
```

It is creating unique variables by incrementing a counter, and than concatenating the variable with the counter. By refactoring `subst`, we should now check for free variables:

```ocaml
let rec subst ~in_term ~variable ~by_term =
  ...
  | Abs (x, t) ->
      if x = variable then Abs (x, t)
      else if free_in ~variable:x ~term:by_term then
        let x' = fresh_var x in
        let t' = subst ~in_term:t ~variable:x ~by_term:(Var x') in
        Abs (x', subst ~in_term:t' ~variable ~by_term)
      else Abs (x, subst ~in_term:t ~variable ~by_term)
```

The first `if` checks if the variable we want to substitute is the same as the one bound by the lambda. If they are equal, we don’t perform substitution inside, because within this lambda, that variable name refers to its parameter, not the outer variable we’re replacing. Otherwise, we check if the `x` appears free in `by_term`. If so, we must rename that parameter before substituting.

Here's the full code:

```ocaml
type term = Var of string | Abs of string * term | App of term * term

let rec to_string ~term =
  match term with
  | Var x -> x
  | Abs (x, body) -> "λ" ^ x ^ "." ^ to_string ~term:body
  | App (t1, t2) -> "(" ^ to_string ~term:t1 ^ " " ^ to_string ~term:t2 ^ ")"

let rec free_in ~variable ~term =
  match term with
  | Var x -> x = variable
  | App (lt, rt) -> free_in ~variable ~term:lt || free_in ~variable ~term:rt
  | Abs (x, body) ->
      if x = variable then false else free_in ~variable ~term:body

let fresh_var =
  let counter = ref 0 in
  fun base ->
    incr counter;
    base ^ string_of_int !counter

let rec subst ~in_term ~variable ~by_term =
  match in_term with
  | Var v -> if v = variable then by_term else Var v
  | App (t1, t2) ->
      App
        ( subst ~in_term:t1 ~variable ~by_term,
          subst ~in_term:t2 ~variable ~by_term )
  | Abs (x, t) ->
      if x = variable then Abs (x, t)
      else if free_in ~variable:x ~term:by_term then
        let x' = fresh_var x in
        let t' = subst ~in_term:t ~variable:x ~by_term:(Var x') in
        Abs (x', subst ~in_term:t' ~variable ~by_term)
      else Abs (x, subst ~in_term:t ~variable ~by_term)

let rec reduce ~term =
  match term with
  | App (Abs (var, body), arg) -> subst ~in_term:body ~variable:var ~by_term:arg
  | App (lt, rt) ->
      let lt' = reduce ~term:lt in
      if lt <> lt' then App (lt', rt) else App (lt, reduce ~term:rt)
  | Abs (var, body) -> Abs (var, reduce ~term:body)
  | Var _ -> term

let () =
  let term = App (Abs ("x", Abs ("y", Var "x")), Var "y") in
  let result = to_string ~term:(reduce ~term) in
  print_endline result

```

And it evaluates to `λy1.y`. The interpreter is already doing beta-reduction and alpha-conversion.

## De Bruijn index

Named variables is kinda messy. Renaming variables, generating fresh ones, checking free vars... Even when two functions are structurally the same, if it is using different variables, our interpreter still treats them as different (e.g. `λx. λy. x`, `λa. λb. a`).

All this process is only used to indicate how far back their binder is.

De Bruijn indices drops all these names, and replace it with a number, indicating how far back their binder is: `λx. λy. x` -> `λ. λ. 1` (for the closest binding). Try some exercicies converting from De Bruijn to named, and from named to De Breuijn.

`λ. λ. λ. 3 (2 1)` becomes `λx. λy. λz. x (y z)`

### Refactoring

The AST has no variables anymore, so we gonna drop all string references:

```ocaml
type term = Var of int | Abs of term | App of term * term
```

```ocaml
let rec to_string ~term =
  match term with
  | Var i -> string_of_int i
  | Abs t -> "λ." ^ to_string ~term:t
  | App (t1, t2) -> "(" ^ to_string ~term:t1 ^ " " ^ to_string ~term:t2 ^ ")"
```

When we substitute a term into another, the indices can move depending on how many binders we cross, and that's why a shift function is needed, to fix the numbers when you relocate a term.

```ocaml
let rec shift ~by ~depth ~term =
  match term with
  | Var i -> if i >= depth then Var (i + by) else Var i
  | Abs t ->
      let depth' = depth + 1 in
      Abs (shift ~by ~depth:depth' ~term:t)
  | App (t1, t2) -> App (shift ~by ~depth ~term:t1, shift ~by ~depth ~term:t2)
```

The `depth` parameter tracks how many binders we've gone under. When we find a variable with index `i >= depth`, it means this variable refers to something outside our current context, so we need to adjust it by `by` amount.

If we have `λ. 0` and want to insert it into another context, we need to shift its free variables so they still point to the right binders.

```ocaml
let rec subst ~index ~arg ~term =
  match term with
  | Var i -> if i = index then arg else Var i
  | Abs t ->
      let index = index + 1 in
      Abs (subst ~index ~arg:(shift ~by:1 ~depth:0 ~term:arg) ~term:t)
  | App (t1, t2) -> App (subst ~index ~arg ~term:t1, subst ~index ~arg ~term:t2)
```

We're checking if the variable index matches the one we want to replace. When we go under a lambda, we increment the index we're looking for, and shift the argument up by 1 so its free variables still refer to the correct binders.

```ocaml
let rec reduce ~term =
  match term with
  | App (Abs body, arg) ->
      shift ~by:(-1) ~depth:0
        ~term:(subst ~index:0 ~arg:(shift ~by:1 ~depth:0 ~term:arg) ~term:body)
  | App (t1, t2) ->
      let t1' = reduce ~term:t1 in
      if t1 <> t1' then App (t1', t2) else App (t1, reduce ~term:t2)
  | Abs t -> Abs (reduce ~term:t)
  | _ -> term
```

About the case `App (Abs body, arg)`:

1. First we shift the argument up by 1: `shift ~by:1 ~depth:0 ~term:arg`. It is about to place it inside the lambda body, where all free variables are one level deeper

2. Then substitute it at index 0: `subst ~index:0 ~arg:... ~term:body`. Index 0 refers to the immediately bound variable.

3. Then shift down by -1: `shift ~by:(-1) ~depth:0`. After substitution, we're removing one lambda, so we need to decrease all free variable indices.

The rest is similar to the named version, we're reducing from left to right (normal-order), and reducing inside abstractions.

To fully normalize a term:

```ocaml
let rec normalize ~term =
  let t = reduce ~term in
  if term = t then term else normalize ~term:t
```

This keeps reducing until the term doesn't change anymore

### Testing with Church numerals

Now let's test our interpreter with Church numerals.
Two is `λf.λx. f (f x)`, which in De Bruijn notation becomes `λ. λ. 1 (1 0)`:

```ocaml
let two = Abs (Abs (App (Var 1, App (Var 1, Var 0))))
```

And the successor function `λn.λf.λx. f (n f x)` becomes `λ. λ. λ. 1 (2 1 0)`:

```ocaml
let succ = Abs (Abs (Abs (App (Var 1, App (App (Var 2, Var 1), Var 0)))))
```

```ocaml
let () =
  let two = Abs (Abs (App (Var 1, App (Var 1, Var 0)))) in
  let succ = Abs (Abs (Abs (App (Var 1, App (App (Var 2, Var 1), Var 0))))) in
  print_endline (to_string ~term:(normalize ~term:(App (succ, two))))
```

Output: `λ.λ.(1 (1 (1 0)))`

This is the Church numeral for 3, which is `λf.λx. f (f (f x))`. It applies `f` three times to `x`



- https://en.wikipedia.org/wiki/Semantics_(programming_languages)
- https://en.wikipedia.org/wiki/Computable_function
- https://en.wikipedia.org/wiki/Effective_method
- https://en.wikipedia.org/wiki/Model_of_computation
- https://en.wikipedia.org/wiki/Computability_theory
- https://en.wikipedia.org/wiki/Turing_degree
