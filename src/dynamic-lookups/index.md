# Dynamic Lookups

![](./top.webp)

*What if I don't know what I want to look up?*

## Dynamic Lookups

The previous section introduced the concept of lookups.
Namely static lookups, in which the values to look up are fixed at circuit generation time
and the only freedom afforded to the prover is to choose the entries to look up.
This is fine if you know all the values you want to look up at compile time
and the set is small enough (e.g. the XOR table of 8-bit values).
But what if you don't? Or maybe the table would be too large to encode explicitly?

In this section we will introduce the concept of dynamic lookups.
It is a natural extension of the static lookups we have seen so far:
rather than looking up rows in `Constant` columns we will look up rows in `Advice` columns.
Such dynamic lookups are widely used in machine emulation, let us start by getting a grasp of why such lookups are useful.

## Motivation

The instruction set architecture (ISA) of a CPU defines the set of potential instructions that the processor can execute.
This set can be rather large, for example the x86-64 ISA has over 1000 instructions,
for the EVM (Ethereum Virtual Machine) this number is a more modest [140](https://www.evm.codes/) instructions but includes very complex operations like `KECCAK256`.
A naive approach to implementing a CPU in a zkSNARK would be to create a circuit for each instruction,
then run every possible instruction in parallel and multiplex the result.
This requires executing every possible instruction at every cycle of the CPU.
Without lookups, this is essentially what people did in the early days of zkSNARKs for machine emulation,
which in turn restricted the size and complexity of the ISA that could be emulated.

Dynamic lookups allow us to avoid this naive approach as follows:

1. We can create a table for each instruction. The table simply contains inputs/outputs for the instruction,
for instance a table for the `ADD` instruction would contain rows of the form:

$$(a, b, a + b)$$

2. When emulating the CPU, we can then do a lookup in each table to retrieve the result for every possible instruction and then multiplex the results.

The key insight is that we need only a single row in the table for the instruction we are actually executing:
all the other lookups can be "nopped out" and the result of the lookup is ignored.
If every instruction previously required a separate circuit with $m$ gates and we have $n$ instructions,
the dynamic lookup approach requires only $n$ tables with $m$ rows each whereas the original approach would require \\( n \cdot m \\) gates.

## Example: Conditional Hashing

The example that we are going to explore is a gate that conditionally hashes a value, i.e.

$$
\mathsf{ConditionalHash}(x, b) = \begin{cases}
\mathsf{Hash}(x) & \text{if } b = 1 \\\\
0 & \text{if } b = 0 \\\\
\end{cases}
$$

The goal is to only "pay" for the hash operation if \\( b = 1 \\):
there will be some a priori bound on the maximum number of hash operations that can be performed,
but we don't know ahead of time where / how many of these operations will be performed.
We want to incur a cost that is proportional to this upper bound.

### A Naive Approach

Before we proceed, let us consider a naive approach to this problem, without dynamic lookups.
The baseline solution to this problem would be to create a circuit that hashes the value unconditionally,
then use a multiplexer to select the output of the hash operation if the condition is true:

$$
\mathsf{ConditionalHash}(x, b) = \mathsf{Hash}(x) \cdot b
$$

As hinted above, the issue with this approach is that the hash operation is always performed,
even if the condition is false: we need to generate a circuit for \\( \mathsf{Hash}(x) \\) and assign the witness, even when \\( b = 0 \\).
So if, for instance, you have a CPU which *may* compute a hash at every cycle,
the circuit of the CPU would have to include the hash operation at every cycle even if the hash is not computed (e.g. if a simple arithmetic operation is performed instead).

In the EVM, the hash might be keccak256 and the condition might be the result of a comparison between the current instruction and the `KECCAK256` opcode.
But in order to keep things simple, we will use a simplified and round-reduced variant of the "Poseidon" hash function instead. This variant is not secure for cryptographic use!

### The Poseidon Hash Function

```admonish warning
The simplified Poseidon used in this example is **not secure** for cryptographic use!

It's a round-reduced variant with the same round function for all rounds.
```

Our simplified Poseidon hash function has a state width of 3 field elements and 8 rounds,
we split the state into two parts: the "RATE" and the "CAPACITY" part:

```rust
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_params}}
```

The `CAPACITY` part is the "internal state" of the hash function, while the message to be hashed is added to the "RATE" part.
As mentioned above there are 8 rounds in the hash function, each round consists of the following operations:

1. AddRoundConstant:
$$(x, y, z) \mapsto (x + a, y + b, z + c)$$
Where a, b, c are constants (and different each round).

2. SBox:
$$
(x, y, z) \mapsto (x^5, y^5, z^5)
$$
The exponent 5 is chosen such that the map is a permutation.

3. Mix:
$$
(x, y, z) \mapsto M (x, y, z)
$$
Where M is a 3x3 matrix (the same for all rounds).

To hash a message \\( (x, y) \\) we initialize the state with \\( (x, y, 0) \\) and then apply the 8 rounds.

The hash digest is the first element of the final state:

$$
\mathsf{Hash}(x, y) = \mathsf{output}
\text{ where }
(\mathsf{output}, ?, ?) = \mathsf{PoseidonPermutation}(x, y, 0)
$$

### The Poseidon Table

We are going to have a table which contains every invocation of the Poseidon hash function and all their intermediate rounds.
The table will contain the following columns:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_table}}
```

Some explanation is in order:

- `matrix` is the fixed matrix M from the `Mix` operation.
- `round_constants` are the constants a, b, c from the `AddRoundConstant` operation.
- `flag_start` is a flag that indicates the start of a new hash invocation.
- `flag_round` is a flag that indicates that the Poseidon round should be applied.
- `flag_final` is a flag that indicates that the result is ready.
- `inp1` is the first input to the Poseidon hash function.
- `inp2` is the second input to the Poseidon hash function.
- `rndc` a set of fixed columns containing the round constants.
- `cols` a set of advice columns storing the state of the Poseidon hash function.

As a table we are going to fill it out like this:

| flag_start | flag_round | flag_final | inp1 | inp2 | rndc1 | rndc2 | rndc3 | col1 | col2 | col3 |
|------------|------------|------------|------|------|-------|-------|-------|------|------|------|
| 1          | 1          | 0          | x    | y    | a1    | b1    | c1    | x    | y    | 0    |
| 0          | 1          | 0          | x    | y    | a2    | b2    | c2    | ...  | ...  | ...  |
| 0          | 1          | 0          | x    | y    | a3    | b3    | c3    | ...  | ...  | ...  |
| 0          | 1          | 0          | x    | y    | a4    | b4    | c4    | ...  | ...  | ...  |
| 0          | 1          | 0          | x    | y    | a5    | b5    | c5    | ...  | ...  | ...  |
| 0          | 1          | 0          | x    | y    | a6    | b6    | c6    | ...  | ...  | ...  |
| 0          | 1          | 0          | x    | y    | a7    | b7    | c7    | ...  | ...  | ...  |
| 0          | 1          | 0          | x    | y    | a8    | b8    | c8    | ...  | ...  | ...  |
| 0          | 0          | 1          | x    | y    | ...   | ...   | ...   | hash | ...  | ...  |

### Constraining the Poseidon Table

There are two types of constraints that we need to add to the Poseidon table:

1. **The `flag_start` constraint**: This resets the state to the input values of the hash function.
2. **The `flag_round` constraint**: This applies the Poseidon round to the state.

Let's take a look at each in turn:

#### The Start Constraint

The start constraint in its totality is as follows:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_start}}
```

What this enforces is:

- `inp1.cur() = col[0].cur()` : load the first input into the first element of the state.
- `inp2.cur() = col[1].cur()` : load the second input into the second element of the state.
- `col[2].cur() = 0` : set the third element of the state to 0.

These constraints are enforced when `flag_start` is set to 1.
It corresponds to a row of this form:

| flag_start | flag_round | flag_final | inp1 | inp2 | rndc1 | rndc2 | rndc3 | col1 | col2 | col3 |
|------------|------------|------------|------|------|-------|-------|-------|------|------|------|
| 1          | ...        | ..         | x    | y    | ...   | ...   | ...   | x    | y    | 0    |

#### The Round Constraint

The round constraint is *substantially* more complex than the start constraint.
It is likely the most complex gate you have encountered so far.
It applies an entire round of the Poseidon hash function to the state,
including the addition of the round constants, the SBox, and the full matrix operation.

So let's break it down into parts.

We start by "reading" the cells of the current row and the next:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_round1}}
```

We then add the round constants to the state:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_round_arc}}
```

Note that this results in an array of `Expression`s: in what follows we are essentially composing constraints
as if we were applying functions to concrete values.
We now apply the SBox to the elements of the state:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_round_sbox}}
```

Finally, we apply the matrix operation to the state (consisting of `Expression`s):

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_round_matrix}}
```

Finally we enforce that the next row is the result of applying this complex transformation to the current row:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_round_constraints}}
```

Overall, this corresponds to a row of the form:

| flag_start | flag_round | flag_final | inp1 | inp2 | rndc1 | rndc2 | rndc3 | col1 | col2 | col3 |
|------------|------------|------------|------|------|-------|-------|-------|------|------|------|
| 0          | 1          | 0          | x    | y    | a1    | b1    | c1    | x'   | y'   | z'   |

Where:

$$(x', y', z') = \text{PoseidonRound}(\mathsf{st} = (x, y, z), \mathsf{rndc} = (a1, b1, c1)) $$

### Filling in the Poseidon Table

To aid in the construction of the Poseidon table, we can define a simple helper function that fills in a single row:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_assign_row}}
```

To assign the entire table we create a function which takes a list of pairs of elements to be hashed and fills the table accordingly.
All it does is hash each pair, round-by-round and fill in the rows of the table sequentially.
There is one special row however: the first row is the all-zero row (all the flags are set to 0 as well),
this is to enable the lookup of a "dummy" value whenever the Poseidon gate is disabled.

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_populate}}
```

Note that when generating the verification key, this function will be run on junk values:
the `inputs` are completely arbitrary and the state of the Poseidon hash function does not matter.
The flags and round constants are fixed however.

## The Poseidon Chip

At this point we have a table, guaranteed to contain correct invocations of the Poseidon hash function.
Now we need to create a chip that can look up the entries (input/output pairs) in the table dynamically
so we can "use the hash function" in a circuit.

### Configuration

Towards this end, we define a chip responsible for looking up the Poseidon table:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_chip}}
```

The fields are mostly self-explanatory, but here is a brief overview:

- `inputs` is a simple way for us to collect all the inputs to the Poseidon hash function we encounter during witness generation.
  Whenever we are asked to hash a pair of values \\( (x, y) \\), we simply hash them out-of-circuit \\( \mathsf{Hash}(x, y) \\) then we add them to this list.

- `sel` is just a selector to turn on this chip at the current offset.

- `tbl` is the Poseidon table we constructed earlier.

- `in1` and `in2` are the inputs to the Poseidon hash function.

- `out` is the output of the Poseidon hash function.

- `on` is the flag that determines whether the Poseidon chip is enabled or not.
  Unlike a selector, which is constant, this can be toggled on and off dynamically (it's an `Advice` column).

The gates of the Poseidon chip are where the magic happens:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_chip_configure}}
```

The first constraint is pretty simple: it simply enforces that `on` is a boolean value whenever `sel` is set to 1.
The `poseidon_lookup` "lookup_any" constraint is where the actual lookup happens:

1. We read `on` at the current offset.
1. We read `sel` at the current offset.
1. We read `in1` at the current offset.
1. We read `in2` at the current offset.
1. We read `out` at the current offset.

We then define `do_lookup = sel * on`, which means that:

$$
\text{do_lookup} = 1 \iff \text{sel} = 1 \land \text{on} = 1
$$

This *dynamic* value will be used to turn the lookup "on" and "off".

In order for us to access the Poseidon table, we need to gain access to its columns,
to do this we follow the approach of PSE and create a little helper which reads the column expressions.
This is done by `let table = tbl.table_expr(cells);` and
inside `table_expr` we simply query the columns at the current offset and return the resulting expressions:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_table_expr}}
```

With that out of the way, we can now define the actual lookup:
when `do_lookup = 1` the lookup requires that the row `(1, inp1, inp2, out)` is in the columns `(flag_final, inp1, inp2, col1)` of the Poseidon table.
In other words: the inputs/output pair is from a row which has been marked by `flag_final = 1` i.e. for which the Poseidon hash function has been completely evaluated.

```admonish question
What do you think would happen if we did not include `flag_final` in the lookup?
```

```admonish answer
If we did not include `flag_final` in the lookup, the prover would be able to lookup any row in the table,
not just the ones corresponding to the *full* evaluation of the Poseidon hash function.
For instance, he could lookup the first row of a Poseidon application:
| flag_start | flag_round | flag_final | inp1 | inp2 | rndc1 | rndc2 | rndc3 | col1 | col2 | col3 |
|------------|------------|------------|------|------|-------|-------|-------|------|------|------|
| 1          | 1          | 0          | x    | y    | a1    | b1    | c1    | x    | y    | 0    |

This row does not represent a complete hash computation, but only the initial state.
The prover could then claim that this initial state is the result of the hash function,
which would yield trivial collisions: \\( \mathsf{Hash}(x, y) = x = \mathsf{Hash}(x, y') \\) for any \\( y' \\).

The flag `flag_final` is used to mark results "ready for consumption" avoiding this issue.
```

### Synthesizing

What remains is to use our new Poseidon chip in a circuit.
To do this we define a function which creates a new region and assigns the output appropriately:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_chip_hash}}
```

The `hash` function stores the inputs into `inputs`,
computes the Poseidon hash function out-of-circuit,
and assigns the inputs/output pairs of the Poseidon chip.
Finally, we turn on the selector `sel` for the Poseidon chip (lookup).
This constrains the inputs/output pair to be in the Poseidon table,
which will only be the case if the Poseidon hash function was correctly evaluated out-of-circuit.

After the last `hash` invocation, we need to finalize the Poseidon chip:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:poseidon_chip_finalize}}
```

Which simply pads the `inputs` vector with dummy values to hash, before populating the Poseidon table.
The overall use of the Poseidon chip is as follows:

```rust,ignore
{{#include ../../halo-hero/examples/conditional-poseidon.rs:test_circuit}}
```

Which should be familiar by now:

- We create three free variables `in1`, `in2`, and `on`.
- We hash the inputs using the Poseidon chip.
- We finalize the Poseidon chip.

Of course in a real application, the variables `in1`, `in2`, and `on` would likely be constrained by other parts of the circuit.

## Exercises

```admonish exercise
**Exercise:**
Implement a Merkle tree chip using lookups into the Poseidon table.
```

```admonish exercise

When opening leaves in a Merkle tree,
you will often end up computing the same hash multiple times.
For instance, consider the tree below:

<svg width="600" height="300" xmlns="http://www.w3.org/2000/svg">
  <!-- Tree structure -->
  <line x1="300" y1="75" x2="150" y2="150" stroke="black"/>
  <line x1="300" y1="75" x2="450" y2="150" stroke="black"/>
  <line x1="150" y1="150" x2="75" y2="225" stroke="black"/>
  <line x1="150" y1="150" x2="225" y2="225" stroke="black"/>
  <line x1="450" y1="150" x2="375" y2="225" stroke="black"/>
  <line x1="450" y1="150" x2="525" y2="225" stroke="black"/>

  <!-- Nodes -->
  <circle cx="300" cy="75" r="30" fill="blue"/>
  <text x="300" y="80" text-anchor="middle" fill="white">Root</text>
  <circle cx="150" cy="150" r="30" fill="green"/>
  <text x="150" y="155" text-anchor="middle" fill="white">A</text>
  <circle cx="450" cy="150" r="30" fill="green"/>
  <text x="450" y="155" text-anchor="middle" fill="white">B</text>
  <circle cx="75" cy="225" r="30" fill="orange"/>
  <text x="75" y="230" text-anchor="middle" fill="white">C</text>
  <circle cx="225" cy="225" r="30" fill="orange"/>
  <text x="225" y="230" text-anchor="middle" fill="white">D</text>
  <circle cx="375" cy="225" r="30" fill="orange"/>
  <text x="375" y="230" text-anchor="middle" fill="white">E</text>
  <circle cx="525" cy="225" r="30" fill="orange"/>
  <text x="525" y="230" text-anchor="middle" fill="white">F</text>
</svg>

And suppose we want to open leaf `D` and `C`.

Verifying the inclusion proof of `C` requires computing:

- \\( \mathsf{Hash}(C, D) \\)
- \\( \mathsf{Hash}(A, B) \\)

And opening `D` requires computing the same hashes:

- \\( \mathsf{Hash}(C, D) \\)
- \\( \mathsf{Hash}(A, B) \\)

Even in the case where `C` and `F` are opened, the hash \\( \mathsf{Hash}(A, B) \\) is still computed twice...

Would it not be nice if we could only compute each hash once inside the circuit?

Well that is actually pretty easy given what we have learned so far.

**Exercise:**
Optimize your Merkle tree chip to only compute each hash once,
deduplicating identical hashes.
```

```admonish hint
If you find this hard to do, you are likely overthinking it :)
```

```admonish exercise
**Exercise:**
Implement an "conditional AES" circuit, where the AES encryption is only performed if a condition is true:

$$
\mathsf{ConditionalAES}(b, k, x) = \begin{cases}
\mathsf{AES128}(k, x) & \text{if } b = 1 \\\\
x & \text{if } b = 0
\end{cases}
$$

This requires combining static and dynamic lookups.
```

```admonish hint
Solve the exercises in the static lookup section first.
```

```admonish hint
You can either combine the table for the AES rounds with the key schedule table,
or do lookups across the two dynamic tables.
The first is the easier option.
```
