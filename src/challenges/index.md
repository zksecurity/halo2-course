# Challenges

![](./top.webp)


```admonish exercise
*Exercise:* Create a circuit which verifies a Sudoku solution.
```

```admonish hint
Use a lookup table to check that every number is an element of {1, 2, 3, 4, 5, 6, 7, 8, 9}.

To check uniqueness, that every row/column/diagonal/3x3 square must contain exactly one of each number.
Use the fact that if you define the polynomial:

`p(X) = `

```