# Lookups

![](./top.webp)

*let's write some ~circuits~ weird machines*


In this installment we will introduce the concept of *lookups*
and we will do so by creating a 
"circuit" that can (very efficiently) match regular expressions.
You might already be thinking that regular expressions are not very efficient to check using a circuit over a finite field,
and you would be right.
Fortunately, lookups enable us to implement this kind of very non-field-friendly functionality in an efficient way.

It might also sound complicated, but in fact the whole circuit fits in just over 200 lines of code.

## Brief Detour: Regular Expressions

```admonish cite
Some people, when confronted with a problem, think "I know, I'll use regular expressions.". \
Now they have two problems.

-- Jamie Zawinski
```


[Ken Thompson](https://en.wikipedia.org/wiki/Thompson%27s_construction) 

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:regex}}
```

| State | Character | Next State |
|-------|-----------|------------|
| ST_A  | a         | ST_A       |
| ST_A  | a         | ST_B       |
| ST_B  | b         | ST_B       |
| ST_B  | b         | ST_C       |
| ST_C  | c         | ST_DONE    |



## Creating a Table of Transitions

In order for us to match against the regular expression, we will create a table of valid transitions.

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:columns}}
```
