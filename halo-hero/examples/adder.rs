// ANCHOR: full
use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Expression, Selector},
    poly::Rotation,
};

use ff::Field;

const STEPS: usize = 5;

// ANCHOR: witness
struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    values: Value<Vec<F>>,
}
// ANCHOR_END: witness

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    q_enable: Selector,
    advice: Column<Advice>,
}

// ANCHOR: without_witnesses
impl<F: Field> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            values: Value::unknown(),
        }
    }
    // ANCHOR_END: without_witnesses

    // ANCHOR: configure
    // ANCHOR: columns
    // ANCHOR: gate
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let q_enable = meta.complex_selector();
        let advice = meta.advice_column();
        // ANCHOR_END: columns

        // define a new gate:
        // next = curr + 1 if q_enable is 1
        meta.create_gate("step", |meta| {
            let curr = meta.query_advice(advice, Rotation::cur());
            let next = meta.query_advice(advice, Rotation::next());
            let q_enable = meta.query_selector(q_enable);
            vec![q_enable * (curr - next + Expression::Constant(F::ONE))]
        });
        // ANCHOR_END: gate

        TestConfig {
            _ph: PhantomData,
            q_enable,
            advice,
        }
    }
    // ANCHOR_END: configure

    // ANCHOR: circuit
    // ANCHOR: region
    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "steps",
            |mut region| {
                // ANCHOR_END: region
                // apply the "step" gate STEPS = 5 times
                for i in 0..STEPS {
                    // assign the witness value to the advice column
                    region.assign_advice(
                        || "assign advice",
                        config.advice,
                        i,
                        || self.values.as_ref().map(|values| values[i]),
                    )?;

                    // turn on the gate
                    config.q_enable.enable(&mut region, i)?;
                }

                // assign the final "next" value
                region.assign_advice(
                    || "assign advice",
                    config.advice,
                    STEPS,
                    || self.values.as_ref().map(|values| values[STEPS]),
                )?;

                Ok(())
            },
        )?;
        Ok(())
    }
    // ANCHOR_END: circuit
}
// ANCHOR_END: full

// ANCHOR: main
fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    // generate a witness
    let start = Fr::from(1337u64);
    let mut values = vec![start];
    while values.len() < STEPS + 1 {
        let last = values.last().unwrap();
        values.push(last + Fr::ONE);
    }

    // run the MockProver
    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        values: Value::known(values),
    };
    let prover = MockProver::run(8, &circuit, vec![]).unwrap();
    prover.verify().unwrap();
}
// ANCHOR_END: main
