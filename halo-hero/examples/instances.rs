use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Expression, Instance, Selector},
    poly::Rotation,
};

use ff::{Field, PrimeField};

const STEPS: usize = 30;

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    fib_seq: Value<Vec<F>>,
    idx_seq: Value<Vec<usize>>,
    flg_seq: Value<Vec<bool>>,
}

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    fib: Column<Advice>,
    flag: Column<Advice>,
    index: Column<Advice>,
    q_fib: Selector,
    instance: Column<Instance>,
}

impl<F: PrimeField> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            fib_seq: Value::unknown(),
            idx_seq: Value::unknown(),
            flg_seq: Value::unknown(),
        }
    }

    // ANCHOR: columns
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let fib = meta.advice_column();
        let flag = meta.advice_column();
        let index = meta.advice_column();
        let q_fib = meta.complex_selector();
        let instance = meta.instance_column();

        meta.enable_equality(fib);
        meta.enable_equality(instance);
        meta.enable_equality(index);
        // ANCHOR_END: columns

        // define a new gate:
        // ANCHOR: gate
        meta.create_gate("fibonacci", |meta| {
            // selector
            let enable = meta.query_selector(q_fib);

            // index in the Fibonacci sequence
            let idx0 = meta.query_advice(index, Rotation(0));
            let idx1 = meta.query_advice(index, Rotation(1));

            // fibonacci sequence
            let w0 = meta.query_advice(fib, Rotation(0));
            let w1 = meta.query_advice(fib, Rotation(1));
            let w2 = meta.query_advice(fib, Rotation(2));

            // indicator
            let bit = meta.query_advice(flag, Rotation(0));
            let not_bit = Expression::Constant(F::ONE) - bit.clone();

            vec![
                // it's a bit (strictly speaking, this is redundant)
                enable.clone() * bit.clone() * not_bit.clone(),
                // apply the Fibonacci rule
                enable.clone() * bit.clone() * (w0.clone() + w1.clone() - w2.clone()),
                enable.clone()
                    * bit.clone()
                    * (idx1.clone() - idx0.clone() - Expression::Constant(F::ONE)),
                // OR, maintain the value / index
                enable.clone() * not_bit.clone() * (w1.clone() - w2.clone()),
                enable.clone() * not_bit.clone() * (idx1.clone() - idx0.clone()),
            ]
        });
        // ANCHOR_END: gate

        TestConfig {
            _ph: PhantomData,
            q_fib,
            fib,
            index,
            flag,
            instance,
        }
    }

    // ANCHOR: synthesize
    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let instances = layouter.assign_region(
            || "fibonacci-steps",
            |mut region| {
                // apply the "step" gate STEPS = 5 times
                let mut fib_cells = Vec::new();
                let mut flg_cells = Vec::new();
                let mut idx_cells = Vec::new();

                for i in 0..STEPS {
                    // turn on the gate
                    config.q_fib.enable(&mut region, i)?;

                    // assign the fibonacci value
                    // ANCHOR: assign_fib
                    fib_cells.push(region.assign_advice(
                        || "assign-fib",
                        config.fib,
                        i,
                        || self.fib_seq.as_ref().map(|v| v[i]),
                    )?);
                    // ANCHOR_END: assign_fib

                    // assign the flag
                    flg_cells.push(region.assign_advice(
                        || "assign-bit",
                        config.flag,
                        i,
                        || {
                            self.flg_seq
                                .as_ref()
                                .map(|v| if v[i] { F::ONE } else { F::ZERO })
                        },
                    )?);

                    // assign the index
                    idx_cells.push(region.assign_advice(
                        || "assign-idx",
                        config.index,
                        i,
                        || self.idx_seq.as_ref().map(|v| F::from_u128(v[i] as u128)),
                    )?);
                }

                // assign the last index

                idx_cells.push(region.assign_advice(
                    || "assign-fib",
                    config.index,
                    STEPS,
                    || {
                        self.idx_seq
                            .as_ref()
                            .map(|v| F::from_u128(v[STEPS] as u128))
                    },
                )?);

                // assign the last two fibonacci values

                fib_cells.push(region.assign_advice(
                    || "assign-fib",
                    config.fib,
                    STEPS,
                    || self.fib_seq.as_ref().map(|v| v[STEPS]),
                )?);

                fib_cells.push(region.assign_advice(
                    || "assign-fib",
                    config.fib,
                    STEPS + 1,
                    || self.fib_seq.as_ref().map(|v| v[STEPS + 1]),
                )?);

                // sanity check

                assert_eq!(flg_cells.len(), STEPS);
                assert_eq!(idx_cells.len(), STEPS + 1);
                assert_eq!(fib_cells.len(), STEPS + 2);

                // enforce instances

                // ANCHOR: return
                Ok([
                    fib_cells[0].cell(),              // start fib0
                    fib_cells[1].cell(),              // start fib1
                    idx_cells[0].cell(),              // start idx0
                    fib_cells.last().unwrap().cell(), // end fib
                    idx_cells.last().unwrap().cell(), // end idx
                ])
                // ANCHOR_END: return
            },
        )?;

        // ANCHOR: constrain
        for (i, cell) in instances.into_iter().enumerate() {
            layouter.constrain_instance(cell, config.instance, i)?;
        }
        // ANCHOR_END: constrain

        Ok(())
    }
    // ANCHOR_END: synthesize
}

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    // ANCHOR: witness_gen
    let fib_steps = 20; // the number of Fibonacci steps we want to prove
    let fib_start0 = Fr::from(1u64); // first Fibonacci number
    let fib_start1 = Fr::from(1u64); // second Fibonacci number

    // generate a witness
    let mut flg_seq = vec![];
    let mut idx_seq = vec![0];
    let mut fib_seq = vec![fib_start0, fib_start1];
    for idx in 1..=STEPS {
        if idx <= fib_steps {
            // generate the Fibonacci sequence
            let f0 = fib_seq[fib_seq.len() - 2];
            let f1 = fib_seq[fib_seq.len() - 1];
            flg_seq.push(true);
            fib_seq.push(f0 + f1);
            idx_seq.push(idx);
        } else {
            // pad the sequences
            flg_seq.push(false);
            fib_seq.push(*fib_seq.last().unwrap());
            idx_seq.push(*idx_seq.last().unwrap());
        }
    }

    // create the circuit
    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        fib_seq: Value::known(fib_seq.clone()),
        flg_seq: Value::known(flg_seq.clone()),
        idx_seq: Value::known(idx_seq.clone()),
    };
    // ANCHOR_END: witness_gen

    // print the assigment
    assert_eq!(flg_seq.len(), STEPS);
    assert_eq!(idx_seq.len(), STEPS + 1);
    assert_eq!(fib_seq.len(), STEPS + 2);
    for i in 0..STEPS + 2 {
        println!(
            "{:3}: {:32} {:5} {:5}",
            i,
            match fib_seq.get(i) {
                Some(v) => format!("{:?}", v),
                None => "-".to_string(),
            },
            match flg_seq.get(i) {
                Some(v) => format!("{:?}", v),
                None => "-".to_string(),
            },
            match idx_seq.get(i) {
                Some(v) => format!("{:?}", v),
                None => "-".to_string(),
            }
        );
    }

    // ANCHOR: run
    // run the MockProver
    let fib_result = fib_seq.last().unwrap().clone();
    let prover = MockProver::run(
        10,
        &circuit,
        vec![vec![
            fib_start0,                       // first Fibonacci number
            fib_start1,                       // second Fibonacci number
            Fr::from_u128(0 as u128),         // start index
            fib_result,                       // claimed result
            Fr::from_u128(fib_steps as u128), // after this number of steps
        ]],
    )
    .unwrap();
    prover.verify().unwrap();
    // ANCHOR_END: run
}
