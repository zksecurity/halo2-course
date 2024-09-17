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
