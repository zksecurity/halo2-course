# Challenges

![](./top.webp)

*"And what is the use of a book," thought Alice, "without pictures or conversations?"*

Because of how PlonK works, it is trivial to add multiple "rounds of interaction" to a proof
in which the prover commits to some values, the verifier sends a challenge and the prover commits to some more values, etc.
This back and forth can be repeated essentially for as many rounds as you like,
Halo2 (as implemented) supports three such "phases" of interaction.

Sticking with the spreadsheet analogy, you can think of the "phases" as
the prover and verifier passing the spreadsheet back and forth while taking turns to fill in values:
first the prover fills out some columns in the spreadsheet, then the verifier fills out some columns,
then the prover fills out some more columns, etc.


## Configuration of Challenges

In Halo2, the "challenges" are used by allocating a `Challenge` which acts very similarly to the columns we have seen so far:
You can think of `Challenge` as a column where *every row contains the same random challenge value*.
They are allocated with a "Phase" using `challenge_usable_after`:

```rust,noplaypen
{{#include ../../halo-hero/examples/challenges.rs:challenge_alloc}}
```

In the example above, we are asking for a challenge that is usable *after the first phase* of the interaction.
The *first phase* is the "default": it is the implicit phase that we have been using to allocate all the `Advice` columns so far:
it is the first time the prover gets to fill in values in the spreadsheet.
This means that only after assigning values to these `Advice` columns, does the prover learn the challenge value (the random value assigned to `alpha`):
so the first phase values cannot depend on the challenge `alpha` in our example.

```admonish question
Stop and think.

Does it make sense to have other column types, besides `Advice`, in any other phases?
```

Before we continue with the challenges, a natural question is: how do we assign `Advice` columns *after* the challenge?
In other words, how do we allow the prover to "respond" to the challenge `alpha`?
It's straightforward: you simply use `meta.advice_column_in(SecondPhase)` instead of `meta.advice_column()` when allocating the column.

```rust,noplaypen
{{#include ../../halo-hero/examples/challenges.rs:phase2_alloc}}
```

These later phase advice columns act just like the first phase advice columns we have seen so far.

As an example, let us create a very simple chip which simply enforces that an advice cell takes the value of the challenge `alpha`.
In that case, configuration should look something like this:

```rust,noplaypen
{{#include ../../halo-hero/examples/challenges.rs:challenge_chip}}
```
This chip takes a `Challenge` and an `Advice` column and enforces that the advice column is equal to the challenge.
For this to work the `Advice` column must be allocated in a phase after the challenge,
in our case, the `SecondPhase`:

```rust,noplaypen
{{#include ../../halo-hero/examples/challenges.rs:configure}}
...
```

## Synthesis of Challenges

So far the prover has had complete knowledge of every value in the circuit from the beginning of proving:
synthesize was able to assign all values in one pass over the circuit.

This now has to change.

We will need to do multiple passes over the circuit.
The reason is that the prover cannot know the challenge value until *after* the first phase.
So we need to do:

- A pass over the circuit, assigning all the `FirstPhase` advice columns.
- Obtain the first challenge value (`alpha` in our example).
- Then do another pass over the circuit, assigning all the `SecondPhase` advice columns which might depend on the challenge value.
- Obtain the second challenge value.
- etc.

Luckily, we already have a nice way to "partially" assign the witness of a circuit:
remember that `Value` is a sum type that can be either `Value::known` or `Value::unknown`.
This is used to access the value of a challenge during synthesis:
the layouter is given a method `get_challenge` which takes a `Challenge` and returns a `Value`,
if the challenge is available during this pass the value will be `Value::known` otherwise it will be `Value::unknown`.

```rust
let chal = layouter.get_challenge(self.challenge);
```

This allows us to compute `Value`s for the second phase: these will be `Value::unknown` during the first pass and `Value::known` during the second pass.
For instance:

```rust
let some_witness = chal.map(|chal| {
    // do something with the challenge
    ...
})
```

In this case, `some_witness` will be a `Value::known` during the second pass and a `Value::unknown` during the first pass
since `chal` is a `Value::unknown` (the `Value` equivalent of `None`).

```rust,noplaypen
{{#include ../../halo-hero/examples/challenges.rs:challenge_access}}
```

When we use it during synthesis, we can now access the challenge value:

```rust,noplaypen
{{#include ../../halo-hero/examples/challenges.rs:synthesize}}
```

## Exercise
So far, so simple. Now let's see what challenges can do for us:
we will use them to create a circuit which efficiently verifies Sudoku solutions
and we will use (two copies) of the Arithmetic chip we developed earlier.

```admonish exercise
Create a circuit which verifies Sudoku solutions.
```

```admonish hint
To verify that every row/column/diagonal/3x3 square must contain exactly one of each number 1-9, you can use the following trick:

Use the fact that for a set \\( C \\) if you define the polynomials:
\\[
  f(X) = \prod_{i=1}^9 (X - i)
\\]
\\[
  g(X) = \prod_{c \in C} (X - c)
\\]

Then
\\[
C = \\{ 1, 2, 3, 4, 5, 6, 7, 8, 9 \\} \iff
f(X) = g(X)
\\]
You can then check \\(f(X) = g(X) \\) by evaluating the polynomials at a random challenge \\( \alpha \\) and enforcing \\( f(\alpha) = g(\alpha) \\)
```

```admonish hint
Build upon the arithmetic chip introduced in earlier exercises.
```

```admonish hint
You might find a `ChallengeChip` useful.
```

```rust,noplaypen
{{#include ../../halo-hero/examples/ex-sudoku.rs:challenge_chip}}
```

```admonish exercise
Fill in an invalid solution to the Sudoku puzzle and verify that the circuit rejects it.
```

## Solutions

Full solution:

```rust,noplaypen
{{#include ../../halo-hero/examples/ex-sudoku.rs}}
```
