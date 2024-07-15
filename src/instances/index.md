# Instances

![](./top.webp)

*Instance, public input, statement, whatever.*

So far, every circuit we have defined has been specific to a statement we wanted the prover to statisfy.
This would be of no use for, e.g. a zk-rollup / validium.
We would need a seperate circuit for every state transition:
in a zk-rollup the circuit shows that a transition between commitments to two adjacent states is valid, without public inputs we would need a seperate circuit for every such pair of commitments. Ouch.
This in-turn would require the verifier to regenerate the verification key for every such new circuit, a very expensive operation, which would defeat the purpose of zk-rollups / validiums : that verification is faster than execution.

The solution is `Instance` columns.
You can think of instances/public inputs as parameterizing the circuit:
for every assignment (known to the verifier) the prover can be asked to provide a witness.
In other, computer science, words:
the SNARK proves satisfiability of some NP relation \\( \mathcal{R} \\):
\\[
  \mathcal{R}(\mathsf{x}, \mathsf{w})) = 1
\\]
Where \\( \mathsf{x} \\), the statement, is known to both parties and \\( \mathsf{w} \\), the witness (advice column assignments), is known only to the prover.
So far we have always had \\( \mathsf{x} \\) be the empty string.

## Instances
