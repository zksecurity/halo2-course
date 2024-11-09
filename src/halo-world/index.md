# Chapter 0: Halo World

![](./top.webp)

*More boilerplate than Java.*

## Setup

For this we are going to need a recent nightly version of Rust:

```
$ rustup default nightly
```

To follow along, create a new Rust project:

```
$ cargo init --bin halo-hero
```

And populate `Cargo.toml` as follows:


```toml
{{#include ../../halo-hero/Cargo.toml}}
```

A few notes on the dependencies, there are two:

- `ff` provides finite field arithmetic traits.
- `halo2_proofs` is the Halo2 library we will use, duh.

There is a sea of different versions of Halo2 out there:

- The original Halo2 by the Zcash foundation: it does not support KZG.
- The Privacy Scaling Explorations (PSE) fork which adds KZG support.
- The Scroll fork which forked from PSE.
- The Axiom fork which forked from PSE.

However, you write circuits in the same way for all of them.

## How To Do Nothing

Let's start by taking a look at the simplest possible circuit: one that does nothing.

Unfortunately, this still requires quite a bit of boilerplate, here is a minimal example:

```rust,noplaypen
{{#include ../../halo-hero/examples/nop.rs}}
```

We will cover the different parts of this code in more detail in the next chapters.

For now we will just make a few observations:

- The `Circuit<F>` trait represents a "top-level" circuit which may be proved or verified.

- The `Circuit<F>` trait is generic over the finite field `F` for which the circuit is defined. \
   In the example above, we implement `Circuit<F>` for every `TestCircuit<F>`.

- You might not always be able to make your circuit generic over every field because it relies on field-specific properties. Perhaps the field needs to be of a certain size, or you might need the ability to convert an integer to a field element -- which requires `F: PrimeField`. \
  In practice, every circuit is over a prime field :)

- The `Circuit<F>` trait has two associated types: `Config` and `FloorPlanner`. \
  The floor planner is not that important, but we will touch on it in the next chapter.

- The `without_witnesses` function is used by the verifier to create an instance of the circuit without any witness data.
  This is required to compute the verification key, which would otherwise require the verifier to know a witness for the circuit in order to generate the verification key required to check the SNARK -- which would partly defeat the purpose.

All of this brings us to the two most central concepts in Halo2. \
In Halo2, there are two steps in creating a circuit:

#### Configuration

This is implemented by the `configure` function.

The high-level task of configuration is to define the available collection of gates in the circuit.

Because Halo2 circuits are much more flexible than traditional arithmetic circuits,
consisting of only addition and multiplication gates,
we need to define the gates that we will use somewhere.
This is done in the `configure` function.
The resulting `Config` can then be used to create instances of the gates (or gadgets) in the circuit.

We will cover this in more detail in the next chapter.

#### Synthesis

After the configuration step, the circuit is synthesized.

This is implemented by the `synthesize` function.

After defining the gates, we need to create a circuit out of them.
This is done in the `synthesize` function.
Which, not surprisingly, takes the `Config` produced by the `configure` function.
It also takes a `Layouter` which is used to store the state of the circuit as it is being built.
Finally, the layouter is then responsible for, well, laying out the circuit in some "physical" space
which we will cover in a lot more detail in the next chapter.
