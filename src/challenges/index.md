# Challenges

![](./top.webp)

```admonish exercise
*Exercise:* Create a circuit which verifies Sudoku solutions.
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
Use fixed columns and equality constraint to enforce the fixed cells in the Sudoku puzzle.
```
