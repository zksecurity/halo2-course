# Bonus: Circuit Architecture

<img src="./top-light-themed.svg" width="100%">

We have seen a number of different techniques.

These exercises will help you explore when to use which technique.

```admonish exercise
**Exercise:** Optimizing multi-MSM.

Suppose the prover wants to demonstrate that he knows the discrete logarithm of points \\( c_1, c_2, c_3 \\)
with respect to a set of base points \\( \vec{G} \\) of length \\( n \\).

$$
\begin{aligned}
  c_1 &= \langle \vec{w_1}, \vec{G} \rangle \\\\
  c_2 &= \langle \vec{w_2}, \vec{G} \rangle \\\\
  c_3 &= \langle \vec{w_3}, \vec{G} \rangle \\\\
\end{aligned}
$$

Doing this naively would require three multi-scalar multiplications of length \\( n \\).
However, there is a better way: this can be achieved with only one multi-scalar multiplication of length \\( n \\) and two scalar multiplications of length \\( 1 \\).

Architect a circuit which achieves this.
```

```admonish hint
Use a challenge
```

```admonish hint
Exploit the linearity of the inner product.
```


```admonish exercise
**Exercise:** The string machine.

We want to design a circuit for efficiently proving different string operations:

- Checking equality.
- String concatenation.
- Computing the length of a string.
- Substring extraction.

We want to support strings of variable length.

Help design chips for these operations.

1. How should strings be represented / stored?
1. Design a gate for checking equality of two strings.
1. Design a gate for concatenating two strings.
1. Design a gate for computing the length of a string.
1. Design a gate for extracting a substring.
1. How could you combine this with our regular expression matching circuit?
```

```admonish hint
Use a column to store the strings.
```

```admonish hint
Use a challenge to compute fingerprints of each string.

Add a gate to ensure that the fingerprints are correctly computed.
```

```admonish hint
Compute on the fingerprints to check that the concatenation is correct.
```

```admonish hint
Add a column containing the length / index of every character in the string.
```

```admonish hint
Decompose the string into three parts: the prefix, the substring, and the suffix.
```

```admonish hint
Use the concatenation gate to extract the substring.
```

```admonish exercise
**Exercise:** Battle Ships

Would it be cool if you could play Battle Ships over the internet without a trusted third party?

Design a circuit which allows two players to play Battle Ships:
the idea is that the state of the game is stored in a commitment (e.g. a Poseidon hash) and the players take turns querying the state of the board.

At a high level, the circuit proves:

1. That the assigment of ships to the board is valid \
e.g. not overlapping and every ship is of the correct length.
2. The position queried by the other player is a hit or a miss.

The questions to ponder are:

1. What public inputs are needed?
1. How can you represent the board?
1. How can you represent the ships?
1. How do you prove that the ships are placed correctly?
1. How do you prove that the position queried is a hit or a miss?
```
