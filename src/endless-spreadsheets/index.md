# Chapter 1: Endless Spreadsheets

![](./top.webp)

*Less circuit, more Excel.*

## Configuration

To get going, we start by adding two *columns* to the circuit during `configuration`:

```rust
{{#include ../../halo-hero/examples/adder.rs:columns}}
```

Each column comes along with a type, in the case above:

- `advice` an "advice" column which the prover can assign values to freely.
- `q_enable` a "selector" column, used to turn gates on and off. \
    It contains constants (that the prover can't change).

This can be visualized as a "spreadsheet" with two columns (and an infinite number of rows):

<center>
    <img src="./columns.svg" width="50%">
</center>

The circuit is defined by fixing certain cells in this "spreadsheet"
and the prover gets to fill in the rest of the spreadsheet.
If we stop here, we have a spreadsheet where:

- The first column is completely unconstrained.
- The second column is column of constant zeros.

For this to do anything useful, we need to define some *gates*.
Gates in Halo2 is kind of a misnomer, it's more useful to think of them as constraints over the cells in this "spreadsheet":
to enforce relationships between the cells (e.g. constraint the first column).

So here is a very simple example of a gate:

```rust
{{#include ../../halo-hero/examples/adder.rs:gate}}
```

This enforces *a global* constraint on the cells in the spreadsheet, i.e. *every row of the spreadsheet*:

- The `advice` cell in the current row : `meta.query_advice(advice, Rotation::cur())`
- Minus the `advice` cell in the next row  : `meta.query_advice(advice, Rotation::next())`
- Plus one : `Expression::Constant(F::ONE)`
- Times the `q_enable` "selector" : `meta.query_selector(q_enable)`

Must be zero. Like this:

![](./gate.svg)

In other words: 

- If `q_enable = 1`, then `next = curr + 1`.
- If `q_enable = 0`, then `next` can be anything.

This is why we need "selector" columns: to turn gates off.

If we did not multiply by a selector (e.g. `q_enable`) the gate *would always be on* and the prover would have to satisfy *every gate* for *every row*.
By turning selectors on/off we can "program" the spreadsheet to enforce different constraints over different rows -- that's the job of the `synthesize` step in Halo2.
Observe that at next next row, the current row is the next row of the previous row:

![](./gate-next.svg)

This means that enabling this gate over a sequence of rows will enforce that the `advice` column contains a sequence
(e, e+1, e+2, ...). 

We will do that in the next section.


```admonish note
Key takeaway: the goal of "configuration" is to define this spreadsheet and the gates (constraints) that act on it.
The goal of the "synthesis" will be to fill in the spreadsheet.
```


```admonish info
The ability to refer to the "next" cell in a column is 
readily applied in Halo2 circuit design, 
when the same function needs to be applied over and over again to an input.
A classical example is a hash function where the gate might constraint a single round of the hash function.
```

## Synthesize

<!--
Synthesis happens during the `synthesize` function from the `Circuit` trait implementation:

```rust
{{#include ../../halo-hero/examples/nop.rs:synthesize}}
```
-->

### Creating Regions

A *set of consecutive rows* are called a *region* in Halo2.
The way they work is as follows: 

1. You ask Halo to create a region.
1. You can then refer to the cells in the region relative to its start.

Creating regions and assigning cells in them is *exactly* the job of the `synthesize` step in Halo2.
The smallest possible region is a single row, the largest is the entire spreadsheet.
In this example, we will have just a single region. 
We create this region using `layouter.assign_region`.

```rust,noplaypen
{{#include ../../halo-hero/examples/adder.rs:region}}
```

A couple natural questions about regions:

```admonish question title="Why does Halo2 need regions?"
  For usability. 
  
  In fact, you could create your circuit by just having a *single giant region* which contains the entire spreadsheet.
  However, this would be very cumbersome to work with, you would have to refer to cells by their absolute position in the spreadsheet
  and manage all the indexing yourself.
  With regions, you can break the spreadsheet into smaller logical pieces, reuse them and give them names for easier debugging etc.
```

```admonish question title="How does Halo2 know where in the spreadsheet the region is?"
  That's easy actually: Halo2 (more specifically the "layouter") decides that for you.
  You can only refer to cells in the region relatively to the regions start,
  which means Halo2 may place the "region" (some subset of rows) anywhere in the spreadsheet.
\
\
  In practice, it will place the region starting at the next available row in the spreadsheet.
```

```admonish question title="How does Halo2 know how many rows are in the region?"
  It figures it out automatically: the number of rows in the region is the largest relative offset you use in the region.
  i.e. if you refer to the 5th row in a region, Halo2 knows that the region has at least 5 rows.
```


```admonish question title="Can regions be next to each other?"
Yes, assuming they don't use the same columns.
```

### Assigning Cells in a Region

We now have a region.

Time to occupy the cells in it, *but first*:
where should the values come from?
i.e. where do we get the values that the prover should fill the `advice` column with?
In other ZK-words, where is the witness?

We feed the witness into `synthesize` by adding a member to the circuit struct:

```rust,noplaypen
{{#include ../../halo-hero/examples/adder.rs:witness}}
```

A natural question arises, what the heck is a `Value`?

```admonish question title="What is a Value?"
A `Value<T>` is a glorified `Option<T>`, nothing more.

It exists to contain values that only the prover knows (the witness), i.e.

- For the verifier the `Value` is `Value::unknown()`
- For the prover it is `Value::known(some_witness)`.

When creating a proof you assign the `Value`'s in the circuit struct with the witness
and run synthesis.
Synthesis then assigns the values in the spreadsheet according to the `Value`'s in the circuit struct.
```

In the case above, it the `Value` contains the witness for the `advice` column: some vector of field elements.
Naturally we must update the `without_witnesses` function from the circuit to return a `Value::unknown()`:

```rust,noplaypen
{{#include ../../halo-hero/examples/adder.rs:without_witnesses}}
```

Okay back to filling out our new region, this looks as follows:

```rust,noplaypen
{{#include ../../halo-hero/examples/adder.rs:circuit}}
```

If assume that:

```rust,noplaypen
self.values = Value::known(vec![1, 2, 3, 4, 5, 6])
```

The assignment is as follows:

<center>
    <img src="./region.svg" width="80%">
</center>

### Exercises

The full code is at the end.

```admonish exercise
*Exercise:*

- Implement the `main` function to create a (valid) `TestCircuit` instance
- Call `prove` on the `TestCircuit` instance.
```

```admonish exercise
*Exercise:*

- Try filling in an invalid value in the `advice` column: e.g. `[1, 2, 3, 4, 5, 5]`.
- Does it fail?
```

```admonish exercise

*Exercise:*

Create a circuit which computes the fibonacci sequence. i.e. enforces next = curr + prev.

- Add a fibonacci gate or change the existing gate to enforce this relation.
- Create valid witness assignments for the circuit: change how `values` is assigned in the `TestCircuit` struct.
```

```admonish hint
You can access the previous row in a region by using `Rotation::prev()`.
```

```rust,noplaypen
{{#include ../../halo-hero/examples/adder.rs:full}}
```