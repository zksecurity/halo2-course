# Chips in Halo2

![](./top.webp)

*You would not implement every method on the same class, would you?*

So far, we have created multiple regions.
We have created regions for addition and multiplication.
We have been stringing them together having a grand old time.
But things are getting a bit unwieldy:
every method has been implemented on the `TestCircuit` struct.

Like writing Java with only a single class...

Throwing everything into the same struct
also hinders reusability: nobody else can use the nice gadgets that we have created,
without buying the whole `TestCircuit` struct.
We need to find a way to make our gadgets more modular.
The pattern we introduce here is that of ["chips"](https://zcash.github.io/halo2/concepts/chips.html).

There is nothing special about a chip, it is just a way to structure our circuits.
Like a class in object-oriented programming, a chip is a way to group related functionality together.
Like a library.

I think we are also ready to graduate to multiple advice columns :)

## An Arithmetic Chip

So let's dive into it.

We will create an `ArithmeticChip` that will implement a small library of arithmetic operations:

```rust,noplaypen
{{#include ../../halo-hero/examples/chips.rs:arithmetic_chip}}
```

As you can see, the chip has:

- A selector `q_mul` to enable the multiplication gate.
- A selector `q_add` to enable the addition gate.
- An advice column `w0` to store the first "input".
- An advice column `w1` to store the second "input".
- An advice column `w2` to store the "output".

An instance of the `ArithmeticChip` can be created at configuration time for the circuit:

```rust,noplaypen
{{#include ../../halo-hero/examples/chips.rs:configure}}
```

Which in turn will invoke a method called `configure` on the `ArithmeticChip`.
We will get to that in a bit.
A natural question at this point arises:

```admonish question title="Why do we define the columns in the configure of the TestCircuit?"

Namely, why not define the `Advice` columns inside `ArithmeticChip::configure`?

The answer is that we could do so, but that it is often useful for different chips to share the same columns.
This is because introducing new columns is relatively expensive:
it increases the size of the proof and the verification time.
Of course `Selector` columns are usually chip-specific:
sharing them would cause different chips to interfere with each other.
```

Configuration of the `ArithmeticChip` should be relatively straightforward to the reader by now:

```rust,noplaypen
{{#include ../../halo-hero/examples/chips.rs:chip-configure}}
```

The only difference from previous examples is that the inputs/outputs are now stored next to each other in the `Advice` columns,
rather than stacked on top of each other in the same column.

At this point we are ready to add some methods to our `ArithmeticChip` to create regions for addition and multiplication:

```rust,noplaypen
{{#include ../../halo-hero/examples/chips.rs:chip-add}}
```

```rust,noplaypen
{{#include ../../halo-hero/examples/chips.rs:chip-mul}}
```

This is essentially just refactoring the code we saw in the previous sections.

Finally, we can use the `ArithmeticChip` in the `TestCircuit` during `synthesize`:

```rust,noplaypen
{{#include ../../halo-hero/examples/chips.rs:synthesize}}
```

That's pretty much chips in a nutshell:
they are simply a collection of functionality, with convenient methods
to create regions.
In the exercises we explore how to use a more complex chip along with custom types.

## Exercises

So the arithmetic chip we have so far is nice and all, but pretty limited/inefficient. A common optimization in PlonKish (e.g. Halo2) is to have a single arithmetic super gate, that can do:

- Addition
- Multiplication
- Bit range checks
- Constant equality checks
- And more!

While also allowing free multiplication/addition by constants.

This allows PlonK to recoup some of the "linear combination is free" advantages exhibited by alternative arithmetizations like R1CS (like Marlin or Groth16).
Such a general arithmetic gate looks as follows:

\\[
\text{c0} \cdot \text{w0} + \text{c1} \cdot \text{w1} + \text{c2} \cdot \text{w2} + \text{cm} \cdot (\text{w0} \cdot \text{w1}) + \text{cc} = 0
\\]

Where:

- `cm` is the multiplication *constant* (`Column<Fixed>`)
- `c0`, `c1`, `c2` are linear *constants* (`Column<Fixed>`)
- `cc` is an additive *constant* (`Column<Fixed>`)
- `w0`, `w1`, `w2` are the advice cells (`Column<Advice>`)

```admonish exercise
Update the `ArithmeticChip` to use the new gate for both addition/multiplication.

This requires introducing a number of new fixed columns.

Where do you think these should be defined?
```

```admonish hint
Setting:

- `c0 = 1`
- `c1 = 1`
- `c2 = -1`
- `cm = 0`
- `cc = 0`

Gives you an addition gate.
```

```admonish exercise
Use the same gate to implement a function fixing a cell to a constant:

Create a function which takes a field element and returns an assigned cell which *must* be equal to the field element.
```

```admonish exercise
Use the same gate to implement a function forcing equality between two cells.
```

```admonish hint
Set:

- `c0 = 1`
- `c1 = -1`
- `c2 = 0`
- `cm = 0`
- `cc = 0`
```

One of the main motivations for the new gate was to reduce the cost of multiplication/addition by constants.
To achieve this we introduce a new struct, called `Variable`:

```rust,noplaypen
{{#include ../../halo-hero/examples/ex-arith.rs:variable}}
```

The value of a `Variable` is `mul * val + add` where `mul` and `add` are fixed field elements and `val` is a cell of `Column<Advice>`. This is useful because we can add/multiply by constants without introducing new regions, simply by changing the `mul` and `add` constants:

```rust,noplaypen
{{#include ../../halo-hero/examples/ex-arith.rs:add-mul-const}}
```

```admonish exercise
Update the `ArithmeticChip` to use the new `Variable` struct for addition/multiplication:
accounting for the new multiplicative and additive constants.
```

```admonish hint
Addition is easy, you just need to change `c0`, `c1`, `cc` appropriately.
```

```admonish hint
Multiplication is a bit tricky, you need to expand the expression:

\\[
(\text{mul}_1 \cdot \text{val}_1 + \text{add}_1) \cdot (\text{mul}_2 \cdot \text{val}_2 + \text{add}_2)
\\]

Then set `c0`, `c1`, `c2`, `cm`, `cc` appropriately to account for the cross terms. Good luck!
```

This gate can also be used to do other things, e.g.
create a fresh variable restricted to a single bit
(the prover must assign the variable either 0 or 1):

```rust,noplaypen
{{#include ../../halo-hero/examples/ex-arith.rs:bit}}
```

```admonish exercise
Implement the `bit` function above.
```

```admonish exercise
Implement a function enforcing equality between two `Variable`'s.
```

## Solution

```rust,noplaypen
{{#include ../../halo-hero/examples/ex-arith.rs}}
```
