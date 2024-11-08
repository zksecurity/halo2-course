# Fixed Columns

![](./top.webp)

*Fixing cells in the spreadsheet.*

It is very useful to be able to fix certain cells in the spreadsheet to a constant value.
This is the goal of the `Column::Fixed` column type which,
as the name suggests, is a column containing values fixed by the verifier which the prover cannot change.

We have in fact already encountered this column type, albeit under a different name:
selector columns, e.g. `q_enable`, are fixed columns with a fixed value of 0 or 1.
The `Column::Fixed` column type is more general, allowing any fixed value.

## Constants in Gates

So now let us use this to enable checks against constants.
One way to use fixed columns is to "program" gates, the simplest example of this is a gate which checks a cell against a constant.

To do so, we introduce a new selector, a new fixed column and a gate to match:

```rust,noplaypen
{{#include ../../halo-hero/examples/fixed.rs:fixed_gate}}
```

Here, `q_fixed * (w0 - c1)` is zero iff:

- `q_fixed` is zero, in which case the gate is disabled.
- `w0` is equal to the cell `c1` in the fixed column.

To use this gate we use an equality constraint to enforce that `w0` matches the `AssignedCell`, assign the constant and turn on the gate:

```rust,noplaypen
{{#include ../../halo-hero/examples/fixed.rs:fixed}}
```

With this new gate, we can finally enforce some non-trivial constraints:

```rust,noplaypen
{{#include ../../halo-hero/examples/fixed.rs:synthesize}}
```

We will see much more complex examples of "programming" gates in subsequent sections. Namely, in the exercises of the "chips" section.

## Constants in Equality Constraints

Another, less common, use of fixed columns is to enforce equality between a cell and a constant by enabling equality constraints on a fixed column.
Using this strategy, we do not need a seperate gate,
we simply define a fixed column and enable equality constraints on it:

```rust,noplaypen
{{#include ../../halo-hero/examples/fixed-alt.rs:fixed_eq}}
```

We can then use this by assigning the desired constant to the fixed column and enforcing equality using `region.constrain_equal`:

```rust,noplaypen
{{#include ../../halo-hero/examples/fixed-alt.rs:fixed}}
```
