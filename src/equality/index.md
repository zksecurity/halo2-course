# Equality Constraints

![](./top.webp)

*So like `=A4` in Excel? Yes exactly.*

## Equality Constraints

Equality constraints allow us to enforce that two arbitrary cells in the spreadsheet are equal.

## Fixing The Soundness Issue

We are going to use these to fix the issue in the previous section.
In order to enforce equality between two cells,
we need to *enable* equality constraints on the columns we want to enforce equality on.
In our example we only have a single advice column:

```rust,noplaypen
{{#include ../../halo-hero/examples/equality.rs:enable_equality}}
```

And we need to add equality constraints between the cells when we assign regions:

```rust,noplaypen
{{#include ../../halo-hero/examples/equality.rs:mul}}
```

Because manually assigning a value from another cell, only to then assign it to another cell, is cumbersome and error-prone,
Halo2 provides a handy function which "copies" one cell to another:


It's simply syntactic sugar for the above code.


## Exercises

The full code is available at the bottom of the page.

```admonish exercise
A
```

```rust,noplaypen
{{#include ../../halo-hero/examples/equality.rs}}
```
