# Static Lookups

![](./top.webp)

*Let's write some ~circuits~ weird machines*


In this installment we will introduce the concept of *lookups*.

Rather than trotting through yet another example with range checks,
we will explore *lookups* by creating a 
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

We then introduce a `lookup` which checks the state transitions:

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:lookup}}
```

Let's break it down. It reads:

- The current state, `st_cur`, from the `st` (state column).
- The next state, `st_nxt`, from the `st` (state column).
- The next character, `ch`, from the `ch` (character column, containing the string to be matched).

The layout looks like this:

![Regex](./regex.svg)

It then multiplies them all by the selector `q_regex`, meaning:

- If `q_regex = 0`, then the lookup is for (0, 0, 0).
- If `q_regex = 1`, then the lookup is for (`st_cur`, `st_nxt`, `ch`).

This 3-tuple is looked up in a table (which we have not populated yet) of valid state transitions.




### Exercises

The full code is at the end.

```admonish exercise
*Exercise:*

- Modify the regular expression to be `a*b+c`, i.e. the change of `a+` to `a*`.

```

```admonish hint
You do not need to change the circuit, only the `REGEX` constant.

However, this exercise is slightly more complicated than it seems at first:
we must somehow be able to transition from matching `a`s without consuming an `a` 
(in case there are zero `a`).
```

```admonish exercise
*Exercise:*

Create a circuit which computes a single round of [AES](https://en.wikipedia.org/wiki/Advanced_Encryption_Standard) encryption.
```

```admonish hint
Use three separate lookup tables to represent:

- The S-box. A table with 256 entries.
- The GF(2^8) multiplication. A table with 256x256 entries.
- The GF(2^8) addition (XOR). A table with 256x256 entries.

You may ignore the key schedule.
```



```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs}}
```