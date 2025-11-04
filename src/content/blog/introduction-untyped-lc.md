---
title: Introduction to the Untyped Lambda Calculus (β-reduction and α-conversion)
pubDate: 2025-11-04
tags:
  - code
  - computer-science
  - math
---

[Lambda calculus](https://en.wikipedia.org/wiki/Lambda_calculus) is a simple programming language, and a model of computation (akin to Turing machines and recursive functions).

It's composed only of abstractions and applications using variables (e.g. `(λx.x) y`). The whole language is made up of _lambda terms_.

Lambda terms can be one of these three things:

- A variable like `x` is a valid lambda term.

- An abstraction `(λx.y)`, or `(λx.t)`, where `x` is it's variable and `y`/`t` is another lambda term. `(λx.x)` could be written in JS as `const id = x => x`. Everything after the _dot_ works as the body of a function, so you could read `λx` as a function receiving `x` as variable. That's why `(λx.x)` is what we call identity function, becase it returns what it receives.

- An application `(t s)` where both `t` and `s` are lamba terms.

Since lambda calculus has only functions, we use [church encoding](https://en.wikipedia.org/wiki/Church_encoding) to model arithmetic, booleans and data structures by representing data types in the lambda calculus.

Church numerals let you represent natural numbers under Church encoding:

- 0 := `λf.λx. x`
- 1 := `λf.λx. f x`
- 2 := `λf.λx. f (f x)`
- 3 := `λf.λx. f (f (f x))`

Each numeral is a function that takes two arguments, `f` and `x`, and applies `f` _n_ times to `x`. That's how we know what is each number, by couting how many times `f` was applied to `x`.

It is just a [higher-order function](https://en.wikipedia.org/wiki/Higher-order_function), in Javascript it would look like this:
```typescript
const zero = f => x => x
const one = f => x => f(x)
const two = f => x => f(f(x))
const three = f => x => f(f(f(x)))
```

If you're not familiar with the notation, you can read it as giving a function `f`, and a value `x`, apply `f` to `x`.

A successor function is quite similar. Since two is `x` applied to `f` two times, to know it's successor, you should apply `f` one more time. So, given a numeral `n`, it returns a numeral that applies `f` (n+1) times.

- succ := `λn.λf.λx.f (n f x)`

Let's break it into small pieces, because it's not that easy to understand at the first time.

```rust
1 := λf.λx. f x
succ := λn.λf.λx. f (n f x)

(λn.λf.λx. f (n f x)) (λf.λx. f x)
    (λf.λx. f ((λf.λx. f x) f x)) // now it is a function that expects f and x
    |           (λx. f x) x
    |               f x
    (λf.λx. f (f x)) // it just becomes the definition of two
```

By reducing the full expression, we're now seeing why succ (1 `λf.λx. f x`) returns 2 (`λf.λx. f (f x)`).

It's possible to do the same for [plus, mult, pow, pred, true, false, and, or, not, ...](https://en.wikipedia.org/wiki/Lambda_calculus#Arithmetic_in_lambda_calculus). Church encoding can also be used to create a pair (2-tuple), lists with their common functions like head, tail, cons...

## Evaluation and Reduction

The process of computing the value of a lambda term is what we call evaluation or reduction, and we will take a look at the β-reduction.

```rust
(λx.x + 1) 5
(5 + 1)
(6)
```

So we're applying a function to an argument by replacing the bound variable with that argument.

A more general rule could be defined as `(λx. M) N → M[x := N]`. Where `M` is the body of the abstraction, and `N`, it's argument. `M[x := N]` replaces all free instances of `x`, by `N` (I did a bunch of reductions by hand to understand well this process). But this rule requires alpha-conversion to avoid variable capure. It happens when a free variable in `N` becomes bound after substitution.

```rust
(λx. λy. x) y
     (λy. y)
```

It changed the meaning of our previous expression. the inner `y` in `(λy. x)` referred to the outer argument `y`. But after substitution, it became bound by the inner `λy`. So it no longer refers to the same variable

With alpha-conversion, we rename bound variables.

```rust
(λx. λy. x) y
(λx. λz. x) y
     (λz. y)
```

We're giving each scope a unique variable name.


```
AND TRUE FALSE
// remember TRUE λx.λy.x and FALSE λx.λy.y

(λp. λq. p q p) TRUE FALSE
     (λq. TRUE q TRUE) FALSE
     TRUE FALSE TRUE
     (λx. λy. x) FALSE TRUE
          (λy. FALSE) TRUE
               FALSE
```

[Try to reduce AND, OR, NOT, IF, etc...](https://en.wikipedia.org/wiki/Lambda_calculus#Logic_and_predicates)

## Starting the interpreter
Well, I think what we know until now is enough to at least start writing an interpreter.

Let's start defining the AST, which one is just a term:

```ocaml
type term = Var of string | Abs of string * term | App of term * term
```

I don't know if you who is reading this are familiar with OCaml, but it is just the type of a term, `... of type` means that a type carries a value, so the type `Var` comes with a string value. If you wanna understand more about it, take a read about variant constructors and algebraic data type.

`Var of string` in a more practial use:
```ocaml
let x = Var "x"
```

Same of the others:
```ocaml
let abs = Abs ("x", Var "x")
```
As you probably have already noticed, this `abs` is the same as `λx.x`. It's possible to write a function to do this prettify for us. This function should receive a term, and transform it to string.

```ocaml
let rec to_string ~term =
  match term with
  | Var x -> x
  | Abs (x, body) -> "λ" ^ x ^ to_string ~term:body
  | App (t1, t2) -> to_string ~term:t1 ^ to_string ~term:t2
```
Nothing fancy, just a recursive function calling itself for terms. The `^` thing is how to concat strings in OCaml.

I'm using labeled arguments just because it is a tutorial and I wanna make it easier to read the code. Otherwise I would write it like this:

```ocaml
let rec to_string t =
  match t with
  | Var x -> x
  | Abs (x, body) -> "λ" ^ x ^ to_string body
  | App (t1, t2) -> to_string t1 ^ to_string t2
```

> Take a look back at your defined `term`. I mentioned the `of ...` notation carries a value. We're using it's value on the pattern matching: `Abs (x, body) -> "λ" ^ x ^ to_string ~term:body`, where `x` is the string and `body` the term.

Now let's take a look at the output by writing an anonymous functions to call `to_string`

```ocaml
let () =
  let term = App (Abs ("x", Var "x"), Abs ("y", Var "y")) in
  let result = to_string ~term in
  print_endline result
```

Output:
```shell
> λxxλyy
```

That's still a bit weird, because it's missing the `.` and `()`:

```ocaml
let rec to_string ~term =
  match term with
  | Var x -> x
  | Abs (x, body) -> "λ" ^ x ^ "." ^ to_string ~term:body
  | App (t1, t2) -> "(" ^ to_string ~term:t1 ^ " " ^ to_string ~term:t2 ^ ")"
```

Output: `(λx.x λy.y)`

Seems good enough.

Now we need to reduce expressions. Let's recap the rule: `(λx. M) N  →  M[x := N]`, so, given the body `M`, replace all free occurrences of `x` with `N`. Let's write it, but in OCaml.

The signature of what we gonna write: `in_term:term -> variable:string -> by_term:term -> term`, so `subst ~in_term:M ~variable:"x" ~by_term:N`:

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

To apply this rule, we also must write a `reduce` function, which gonna call `subst`, so we gonna substitute terms recursively. Also, worth remembering that we're doing [normal-order reduction](https://en.wikipedia.org/wiki/Lambda_calculus#Reduction_strategies). It means we gonna reduce from left to right.

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

Again, idk how much you who are reding this knows about OCaml, so I want to explain a bit this code.
- `| App (Abs (var, body), arg)`: we want to check for the tradicional case. It matches for expressions like `(λx.x) y`. For this case, we can just call `subst`.

- `| App (lt, rt)`: now we need to handle things like `((λx.x)(λy.y))z`. Since we're implementing a normal-order reduction, we gonna first reduce the `lf` (left term). `<>` in OCaml compares two structures, so we're checking if `lt` and `lt'` differs, if so, we're returning an application, but with the left-term reduced. Otherwise, the left-term is already reduced, so we reduce the right one.

- `| Abs (var, body)`: nothing special, we're reducing the only available term to reduce.

Good, here's the code until now:

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

It prints to `y`, which is fine. But let's try to reduce this other expression: `(λx.λy.x)y` (`let term = App (Abs ("x", Abs ("y", Var "x")), Var "y")`). It prints `λy.x`, which is wrong, because we didn't implemented alpha-conversion yet. We need to fix our `subst` `Abs (x, t)` match case.

Let's write a function to check if a variable is free in a given term.
```ocaml
let rec free_in ~variable ~term =
  match term with
  | Var x -> x = variable
  | App (lt, rt) -> free_in ~variable ~term:lt || free_in ~variable ~term:rt
  | Abs (x, body) ->
      if x = variable then false else free_in ~variable:x ~term:body
```

We can check if a variable is free, but we still need to rename it somehow, so let's now write a function to create a new unique name:

```ocaml
let fresh_var =
  let counter = ref 0 in
  fun base ->
    incr counter;
    base ^ string_of_int !counter
```

This one is just a closure and we're creating unique variables by incrementing a counter, and than concating the variable with the counter. In OCaml, this `!` means dereference. So we're getting the ref. value inside counter.

By refactoring `subst`, we should now check for free variables:

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

The first `if` checks if the variable we want to substitute is the same as the one bound by the lambda.
If they are equal, we don’t perform substitution inside, because within this lambda, that variable name refers to its parameter, not the outer variable we’re replacing.

Otherwise, we check if the `x` appears free in `by_term`. If so, we must rename that parameter before substituting.

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

And it evaluates to `λy1.y` now, which is the correct result. The interpreter is already doing beta-reduction and avoiding shadowing by applying alpha-conversion.

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
