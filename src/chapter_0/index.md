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



A few notes on the dependencies:

- `ff` provides finite field arithmetic traits.
- `halo2_proofs` is the Halo 2 library we will use.

There is a sea of different versions of Halo 2 out there:

- The original Halo 2 by the Zcash foundation: it does not support KZG.
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

