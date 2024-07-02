# Multiple Regions

![](./top.webp)

*Okay, circuits have chips, right?*


## Another Gate

Okay new gate time.
Change the `configure` function into:

```rust
{{#include ../../halo-hero/examples/regions.rs:configure}}
```

```admonish question title="Stop-and-Think"

What does this gate do?

Hint: 

- Rotation(0) = Rotation::cur()
- Rotation(1) = Rotation::next()
- Rotation(2) = The Next Next Row
```

To ease the creation of 

```rust
{{#include ../../halo-hero/examples/regions.rs:mul_region}}
```

This function takes two assigned cells and returns a cell that is assigned the product of the two inputs. 

```admonish warning
This code is not safe (yet). 

We will get to that when we explore equality constraints.
```


In order to use this function, we will also need some way to *create* assigned cells.
To do this we create a function which allocates a small (1 row) region, enables no gates and simply returns the cell:

```rust
{{#include ../../halo-hero/examples/regions.rs:wit_region}}
```

## The Issue

As hinted at in the warning, this code is <u>not safe</u>!

The problem is that equality is not enforced between:

- The assigned cells
`lhs`/`rhs` 
- The assigned cells `w0`/`w1`

To fix this, we are going to need equality constraints.