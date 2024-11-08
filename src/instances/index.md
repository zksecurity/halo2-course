# Instances

![](./top.webp)

*Instance, public input, statement, whatever.*

So far, every circuit we have defined has been specific to a statement we wanted the prover to statisfy.
This would be of no use for, e.g. a zk-rollup / validium.
We would need a seperate circuit for every state transition:
in a zk-rollup the circuit shows that a transition between commitments to two adjacent states is valid, without public inputs we would need a seperate circuit for every such pair of commitments. Ouch.
This in-turn would require the verifier to regenerate the verification key for every such new circuit, a very expensive operation, which would defeat the purpose of zk-rollups / validiums : that verification is faster than execution.

The solution is `Instance` columns.
You can think of instances/public inputs as parameterizing the circuit:
for every assignment (known to the verifier) the prover can be asked to provide a witness.
In other, computer science, words:
the SNARK proves satisfiability of some NP relation \\( \mathcal{R} \\):
\\[
  \mathcal{R}(\mathsf{x}, \mathsf{w})) = 1
\\]
Where \\( \mathsf{x} \\), the statement, is known to both parties and \\( \mathsf{w} \\), the witness (advice column assignments), is known only to the prover.
So far we have always had \\( \mathsf{x} \\) be the empty string.

## Instances

An `Instance` column is just a regular column, like an advice column,
but the values in the instance column are provided/known to the verifier;
as opposed to the `Advice` column, which are only known to the prover.

Because `Instance` columns themselves are pretty simple,
we are going to visit some additional techniques in the examples below:
as a bit of a throwback to the very first circuit we defined,
we will define a circuit that takes a index and returns the fibonacci number at that index.
This means that the circuit must be able to a accomodate a variable number of "steps" in the fibonacci sequence.

Our circuit will have 5 columns:

- `fib`: an `Advice` column which will contain the fibonacci sequence.

- `flag`: an `Advice` column which will contain a "flag" for the gate. \
  More details on this later.

- `index`: an `Advice` column which will contain the index of the fibonacci number.

- `q_step`: this is the `Selector` column which turns on the sole gate: the "fibonacci" gate.

- `instance`:  `Instance` column which will contain the index of the fibonacci number we want.

Looks like this:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/instances.rs:columns}}
```

## The Fibonacci Gate

There is a single gate in this circuit, the "fibonacci" gate.
This gate is by far the most complex gate we have defined so far, so hold on to your hats:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/instances.rs:gate}}
```


The layout of the gate is as follows:

![Fibonacci gate](./fib.svg)

This is a gate that essentially allows the prover to choose between two smaller gates:

- Force `bit` to be either 0 or 1.
- If `bit = 1`: calculate the next fibonacci number.
  - `w2 = w0 + w1`
  - `idx1 = idx0 + 1`
- If `bit = 0`: do nothing.
  - `w2 = w0`
  - `idx1 = idx0`

The motivation for this gate is that we want to be able to calculate the fibonacci number at any index:
by setting the `bit` to 0, the prover can choose to stop progressing the fibonacci sequence once it reaches the desired index.

An alternative way to implement this gate would be to use a dynamic lookup,
but we have not covered these yet, stay tuned.

```admonish question
Observe that the gate has a comment about the first constraint being redundant.
Why is this?
```

## The Fibonacci Circuit

The circuit is simply turning on the "fibonacci" gate for some number of rows.
We save the cells from all these assignments, finally we export the intial state and the final state as instances.

We will break it down in the next section, but here it is in all its glory:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/instances.rs:synthesize}}
```

Most of the code is very reminiscent of the circuit we explored in the "Endless Spreadsheets" section very early on.
One small difference is that we save the assigned cells,
e.g. saving the assigned cells of the `fib` column in the `fib_cells` vector:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/instances.rs:assign_fib}}
```

Then at the end return some cells from the region:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/instances.rs:return}}
```

Namely:

- The first two cells of the `fib` column (first two fibonacci numbers).
- The initial index of the first fibonacci number, i.e. 0.
- The final cell of the `fib` column.
- The final index of the last fibonacci number.

We then enforce that the `instance` column is equal to these cells:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/instances.rs:constrain}}
```

This, in a nutshell, is how we use instances.

## The Witness Generation

Let's briefly look at how the prover generates the witness for this circuit:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/instances.rs:witness_gen}}
```

In other words, the prover progresses the fibonacci sequence until
`idx > fib_steps` at which point the prover sets the flag bit to 0
and adds padding until the end of the region.

## The Verifier

The only change to the `MockProver` is that we need to provide the instances:

```rust,no_run,noplaypen
{{#include ../../halo-hero/examples/instances.rs:run}}
```

```admonish note
The instances taken by the verifier is a `Vec<Vec<F>>`.

This is to support multiple `Instance` columns: one vector of values per column.
```
