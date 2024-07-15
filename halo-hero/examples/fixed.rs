use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Fixed, Selector},
    poly::Rotation,
};

use ff::{Field, PrimeField};

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    secret: Value<F>,
}

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    q_mul: Selector,
    q_fixed: Selector,
    advice: Column<Advice>,
    fixed: Column<Fixed>,
}

impl<F: PrimeField> TestCircuit<F> {
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
                config.q_mul.enable(&mut region, 0)?;

                // enforce equality between the w0/w1 cells and the lhs/rhs cells
                region.constrain_equal(w0.cell(), lhs.cell())?;
                region.constrain_equal(w1.cell(), rhs.cell())?;

                Ok(w2)
            },
        )
    }

    /// This region occupies 1 row.
    fn free(
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

    // ANCHOR: fixed
    /// This region occupies 1 row.
    fn fixed(
        config: &<Self as Circuit<F>>::Config,
        layouter: &mut impl Layouter<F>,
        value: F,
        variable: AssignedCell<F, F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "fixed",
            |mut region| {
                variable.copy_advice(
                    || "assign variable", //
                    &mut region,
                    config.advice,
                    0,
                )?;
                region.assign_fixed(
                    || "assign constant",
                    config.fixed, //
                    0,
                    || Value::known(value),
                )?;

                // turn the gate on
                config.q_fixed.enable(&mut region, 0)?;
                Ok(())
            },
        )
    }
    // ANCHOR_END: fixed
}

impl<F: PrimeField> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            secret: Value::unknown(),
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let q_mul = meta.complex_selector();
        let advice = meta.advice_column();

        // enable equality constraints
        meta.enable_equality(advice);

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
            let q_enable = meta.query_selector(q_mul);
            vec![q_enable * (w0 * w1 - w3)]
        });

        // ANCHOR: fixed_gate
        // selector for the fixed column
        let q_fixed = meta.complex_selector();

        // add a new fixed column
        let fixed = meta.fixed_column();

        meta.create_gate("equal-constant", |meta| {
            let w0 = meta.query_advice(advice, Rotation::cur());
            let c1 = meta.query_fixed(fixed, Rotation::cur());
            let q_fixed = meta.query_selector(q_fixed);
            vec![q_fixed * (w0 - c1)]
        });
        // ANCHOR_END: fixed_gate

        TestConfig {
            _ph: PhantomData,
            q_mul,
            q_fixed,
            advice,
            fixed,
        }
    }

    // ANCHOR: synthesize
    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let a = TestCircuit::<F>::free(&config, &mut layouter, self.secret.clone())?;

        // a2 = a * a = a^2
        let a2 = TestCircuit::<F>::mul(
            &config,
            &mut layouter, //
            a.clone(),
            a.clone(),
        )?;

        // a3 = a2 * a = a^3
        let a3 = TestCircuit::<F>::mul(
            &config,
            &mut layouter, //
            a2.clone(),
            a.clone(),
        )?;

        // a5 = a3 * a2 = a^5
        let a5 = TestCircuit::<F>::mul(
            &config,
            &mut layouter, //
            a3.clone(),
            a2.clone(),
        )?;

        // fix the value 1337
        TestCircuit::<F>::fixed(
            &config,
            &mut layouter, //
            F::from_u128(4272253717090457),
            a5,
        )?;

        Ok(())
    }
    // ANCHOR_END: synthesize
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
