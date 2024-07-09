# Chips in Halo2

![](./top.webp)

*You would not implement every method on the same class, would you?*

So far, we have created multiple regions.
We have created regions for addition and multiplication.
We have been stringing them together.
But things are getting a bit unwieldy:
every method has been implemented on the `TestCircuit` struct.

It also hinders reusability: nobody else can use the nice gadgets that we have created,
without buying the whole `TestCircuit` struct.
We need to find a way to make our gadgets more modular.
The pattern we introduce here is that of ["chips"](https://zcash.github.io/halo2/concepts/chips.html).

There is nothing special about a chip, it is just a way to structure our circuits.
Like a class in object-oriented programming, a chip is a way to group related functionality together:
you can write Java by having only a single class, but it becomes unwieldy.

I think we are also ready to graduate to multiple advice columns :)

## An Arithmetic Chip

```rust,noplaypen
{{#include ../../halo-hero/examples/chips.rs:arithmetic_chip}}
```


## Exercises

So the arithmetic chip we have so far is nice and all, but pretty limited/inefficient.

A common optimization in PlonKish is to have a single arithmetic gate, that can do:

- Multiplication
- Addition
- Bit range checks
- Constant equality checks
- And more!

While also allowing free multiplication/addition by constants.
This allows PlonK to recoup some of the "linear combination is free" advantages exhibited by alternative arithmetizations like R1CS (like Marlin or Groth16).

This general arithmetic gate looks as follows:

```
cm * w0 * w1 + c0 * w0 + c1 * w1 + c2 * w2 + cc = 0
```

Where:

- `cm` is the multiplication *constant* (`Column<Fixed>`)
- `c0`, `c1`, `c2` are linear *constants* (`Column<Fixed>`)
- `cc` is a *constant* (`Column<Fixed>`)
- `w0`, `w1`, `w2` are the wires (`Column<Advice>`)

```admonish exercise
**Exercise:** Update the `ArithmeticChip` to use the new gate for addition/multiplication.
```
