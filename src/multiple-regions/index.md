# Regions in Halo2

![](./top.webp)

*Okay, circuits have chips, right?, oh! and regions!*

Up till this point we have had *a single large region*,
although this is a useful technique because it allows easy "repeating" computation, by referring to neighboring cells,
it would be a hard way to build circuits in general:
normally, we like to compartmentalize and compose smaller things into larger things.
Whether in software engineering or hardware design, this is a common pattern,
and Halo2 circuit development is no different.

Using (multiple) regions will allow us to create distinct "logical" units that we can compose.
In fact, what people usually refer to as a "gate" (think addition/multiplication gates)
are in fact more analogous to regions with certain gates enabled in Halo2.
This will enable us to create more complex circuits in a more modular way by building up a library of "gadgets" which we can compose to obtain more and more complex behavior.

## Another Gate


Okay new gate time.
Change the `configure` function into:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/regions.rs:configure}}
```

```admonish question title="Stop-and-Think"

What does this gate do?

Hint: 

- Rotation(0) = Rotation::cur()
- Rotation(1) = Rotation::next()
- Rotation(2) = The Next Next Row
```

To ease the creation of the multiplication gate, we will add a function that allocates a new separate region for a multiplication gate:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/regions.rs:mul_region}}
```

This function takes two assigned cells and returns a cell that is assigned the product of the two inputs. 

```admonish warning
This code is not safe (yet)! Can you see why?

We will get to that when we explore equality constraints.
```


In order to use this function, we will also need some way to *create* assigned cells.
To do this we create a function which allocates a small (1 row) region, enables no gates and simply returns the cell:

```rust
{{#include ../../halo-hero/examples/regions.rs:wit_region}}
```

With this we can start writing circuits in style:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/regions.rs:synthesize}}
```

So far this circuit is not that useful: we are not checking the result of the multiplications against anything.
We will get to that in the next couple of sections.

## The Issue

As hinted at in the warning, this code is <u>not safe</u>!

The problem is that equality is not enforced between:

- The assigned cells
`lhs`/`rhs` 
- The assigned cells `w0`/`w1`

To fix this, we are going to need equality constraints, which we will explore in the next section.


## Exercises

```admonish exercise
**Exercise:** Implement an addition gate in the same style as the multiplication gate.
```
