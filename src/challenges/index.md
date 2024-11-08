# Challenges

![](./top.webp)

*“And what is the use of a book,” thought Alice, “without pictures or conversations?”*

Because of how PlonK works, it is trivial to add multiple "rounds of interaction" to a proof
in which the prover commits to some values, the verifier sends a challenge and the prover commits to some more values, etc.
This back and forth can be repeated essentially for as many rounds as you like,
Halo2 (as implemented) supports three such "phases" of interaction.

## Challenges in Halo2

In Halo2, the "challenges" are accessed similarly to columns:


## Exercises

```admonish exercise
Create a circuit which verifies Sudoku solutions.
```

```admonish hint
To that every row/column/diagonal/3x3 square must contain exactly one of each number 1-9, you can use the following trick:

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
You might find the a `ChallengeChip` useful.
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
