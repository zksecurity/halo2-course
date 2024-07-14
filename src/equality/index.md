# Equality Constraints

![](./top.webp)

*So like `=A4` in Excel? Yes exactly.*

## Equality Constraints

Equality constraints allow us to enforce that two arbitrary cells in the spreadsheet are equal.

## Fixing The Soundness Issue

We are going to use these to fix the issue in the previous section:
enforcing that `w0` and `w1` are equal to `lhs` and `rhs` respectively.
In order to enforce equality between cells,
we need to *enable* equality constraints on the columns we want to enforce equality on.

In our example we only have a single advice column:

```rust,noplaypen
{{#include ../../halo-hero/examples/equality.rs:enable_equality}}
```

And we need to add equality constraints between the cells when we assign regions:

```rust,noplaypen
{{#include ../../halo-hero/examples/equality.rs:mul}}
```

Because manually assigning a value from another cell, only to then enforce equality it to the same cell, is very common, cumbersome and error-prone,
Halo2 provides a handy function which "copies" one cell to another:

```rust,noplaypen
{{#include ../../halo-hero/examples/equality.rs:copy}}
```

It's simply syntactic sugar for the above code.

## Exercises

The full code is available at the bottom of the page.

```admonish exercise
**Exercise:**
Try reimplementing the attack from the previous section.

Does the circuit reject the assigment now?
```

```rust,noplaypen
{{#include ../../halo-hero/examples/equality.rs}}
```
