---
title: Is Data Struct about memory?
pubDate: 2026-07-27
tags:
    - math
    - computer-science
---

If you think about data structures, you might immediately relate them to memory. Still, it is not the only way to think about them, and it is actually completely possible to discuss data structures without ever involving memory.

It is possible to store things in math, and a data structure (speaking mathematically) is a way to organize, manage, and store data. That’s the point of things like [matrices](<https://en.wikipedia.org/wiki/Matrix_(mathematics)>) or [sets](<https://en.wikipedia.org/wiki/Set_(mathematics)>); In practice, storing things is about organizing things. When we’re using some data structure, there is a collection of values that we can perform operations on, like calling functions. So, it is, in essence, an algebraic structure of data.

# Algebraic structure

An [algebraic structure](https://en.wikipedia.org/wiki/Algebraic_structure) is a set of elements with one or more operations that satisfy some specific properties. An example is [Boolean Algebra](https://en.wikipedia.org/wiki/Boolean_algebra) (a tuple); it carries five operations: AND, OR, NOT, and two distinguished constants (∧, ∨, ¬, 0, 1), and its properties are described [through axioms](https://en.wikipedia.org/wiki/Boolean_algebra#Axiomatizing_Boolean_algebra) (commutativity, associativity, distributivity, identity, and complement). That way of thinking about it led me to reduce the initial problem to something more general and completely abstract. The point that I want to bring is that you do not need to think about any implementation, whether software or hardware, to think about data structures, because they’re essentially an abstract mathematical topic. The only requirement is data, and [data](https://en.wikipedia.org/wiki/Data), in math, is just a set of elements.

# Problem modeling

The reason I believe that makes people avoid thinking abstractly is the lack of the relevant mental model for a given topic. You can think about "model" as an abstraction step, to make something fit a set of primitives. And so, for example, you can use math to model a set of different problems. The same logic applies to Computer Science, since it uses math to model computation problems, which makes all computation problems essentially mathematical problems.

So literally every topic in Computer Science, including data structures, can be discussed as a math problem, which lets you use all the constraints that are already present in math. The ability to understand that different topics might use the same models is pretty useful for discussing and thinking about things, because it lets you reduce your problems to something simpler and solve them generically.

All this is about abstracting things to make them fit a specific model and making it possible to use all pre-existing tools on that model to interact with that thing.
