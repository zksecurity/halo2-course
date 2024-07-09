# Fixed Columns

![](./top.webp)

*Fixing cells in the spreadsheet.*

It is very useful to be able to fix certain cells in the spreadsheet to a constant value.
This is the goal of the `Column::Fixed` column type which,
as the name suggests, is a column containing values fixed by the verifier which the prover cannot change.

We have in fact already encountered this column type, albeit under a different name:
selector columns, e.g. `q_enable`, are fixed columns with a fixed value of 0 or 1.
The `Column::Fixed` column type is more general, allowing any fixed value from the field.

That is it, that is all there is to it.

## Constant Checks

So now let us use this to enable checks against constants:

