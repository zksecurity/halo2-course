// A = (a_1, ..., a_n)
// B = (b_1, ..., b_n)

// We              "Verifier"
//   -- Com(A, B) -> "write it down"
//
//   <-- x ---       "the dice"
//
// More Computation Here
//
// \sum_i x^i a_i = \sum_i x^i b_i
//
// a_n
// * x + a_{n-1}
// * x + a_{n-2}
// * x + ...
// = \sum_i x^i b_i
//
// b_n
// * x + b_{n-1}
// * x + b_{n-2}
// * x + ...
// = \sum_i x^i b_i

// A = { a_1, ..., a_n }
// B = { b_1, ..., b_n }
//
// A = B as sets.
// a_i = b_i for all i. <-- won't work.
//
// f(X) = \prod (X - a_i) // zero at every a_i
// g(X) = \prod (X - b_i) // zero at every b_i
//
// Clear:
// A = B --> f(X) = g(X)
//
// Also:
// \forall a_i. f(a_i) = 0
// \forall b_i. g(b_i) = 0
//
// f(X) - g(X) = h(X) != 0 <--
//

// f(X) = \prod (X - a_i) // zero at every a_i
// g(X) = \prod (X - b_i) // zero at every b_i

// (3, 2, 1, 4, 6, 5, 7, 8, 9)
// a_1, a_2, a_3, a_4, a_5, a_6, a_7, a_8, a_9
//
// Is it a perm of this:
// (1, 2, 3, 4, 5, 6, 7, 8, 9)
//
// g(X) = \prod_{i = 1}^9 (X - i)
// f(X) = \prod_{i = 1}^9 (X - a_i)
//
// <-- x (challenge) --
//
// f(x) = g(x) -- g(X) = f(X)
//
// (f - g)(X) is at most degree 9.
//
fn main() {}
