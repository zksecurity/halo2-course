use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Selector},
    poly::Rotation,
};

use ff::Field;

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    secret: Value<F>,
}

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    q_enable: Selector,
    advice: Column<Advice>,
}

impl<F: Field> TestCircuit<F> {
    // ANCHOR: mul
    /// This region occupies 3 rows.
    fn mul(
        config: &<Self as Circuit<F>>::Config,
        layouter: &mut impl Layouter<F>,
        lhs: AssignedCell<F, F>,
        rhs: AssignedCell<F, F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "mul",
            |mut region| {
                let w0 = lhs.value().cloned();
                let w1 = rhs.value().cloned();
                let w2 =
                    w0 //
                        .and_then(|w0| w1.and_then(|w1| Value::known(w0 * w1)));

                let w0 = region.assign_advice(|| "assign w0", config.advice, 0, || w0)?;
                let w1 = region.assign_advice(|| "assign w1", config.advice, 1, || w1)?;
                let w2 = region.assign_advice(|| "assign w2", config.advice, 2, || w2)?;
                config.q_enable.enable(&mut region, 0)?;

                // ANCHOR: enforce_equality
                // enforce equality between the w0/w1 cells and the lhs/rhs cells
                region.constrain_equal(w0.cell(), lhs.cell())?;
                region.constrain_equal(w1.cell(), rhs.cell())?;
                // ANCHOR_END: enforce_equality

                Ok(w2)
            },
        )
    }
    // ANCHOR_END: mul

    /// This region occupies 1 row.
    fn unconstrained(
        config: &<Self as Circuit<F>>::Config,
        layouter: &mut impl Layouter<F>,
        value: Value<F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "free variable",
            |mut region| {
                let w0 = value;
                let w0 = region.assign_advice(|| "assign w0", config.advice, 0, || w0)?;
                Ok(w0)
            },
        )
    }
}

impl<F: Field> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            secret: Value::unknown(),
        }
    }

    // ANCHOR: enable_equality
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        // let q_enable = meta.fixed_column();
        let q_enable = meta.complex_selector();
        let advice = meta.advice_column();

        // enable equality constraints
        meta.enable_equality(advice);
        // ANCHOR_END: enable_equality

        // ANCHOR: new_gate
        // define a new gate:
        //
        // Advice
        // |      w0 |
        // |      w1 |
        // | w0 * w1 |
        meta.create_gate("vertical-mul", |meta| {
            let w0 = meta.query_advice(advice, Rotation(0));
            let w1 = meta.query_advice(advice, Rotation(1));
            let w3 = meta.query_advice(advice, Rotation(2));
            let q_enable = meta.query_selector(q_enable);
            vec![q_enable * (w0 * w1 - w3)]
        });
        // ANCHOR: new_gate

        TestConfig {
            _ph: PhantomData,
            q_enable,
            advice,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let a = TestCircuit::<F>::unconstrained(&config, &mut layouter, self.secret.clone())?;

        // do a few multiplications
        let a2 = TestCircuit::<F>::mul(&config, &mut layouter, a.clone(), a.clone())?;
        let a3 = TestCircuit::<F>::mul(&config, &mut layouter, a2.clone(), a.clone())?;
        let _a5 = TestCircuit::<F>::mul(&config, &mut layouter, a3.clone(), a2.clone())?;

        Ok(())
    }
}

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    // run the MockProver
    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        secret: Value::known(Fr::from(1337u64)),
    };
    let prover = MockProver::run(8, &circuit, vec![]).unwrap();
    prover.verify().unwrap();
}
