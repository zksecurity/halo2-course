use std::marker::PhantomData;

use halo2_proofs::{
    arithmetic,
    circuit::{AssignedCell, Chip, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Selector},
    poly::Rotation,
};

use ff::Field;

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    secret: Value<F>,
}

// ANCHOR: arithmetic_chip
#[derive(Clone, Debug)]
struct ArithmeticChip<F: Field> {
    _ph: PhantomData<F>,
    q_mul: Selector,
    q_add: Selector,
    w0: Column<Advice>,
    w1: Column<Advice>,
    w2: Column<Advice>,
}
// ANCHOR_END: arithmetic_chip

impl<F: Field> ArithmeticChip<F> {
    fn configure(
        meta: &mut ConstraintSystem<F>,
        w0: Column<Advice>,
        w1: Column<Advice>,
        w2: Column<Advice>,
    ) -> Self {
        let q_mul = meta.complex_selector();
        let q_add = meta.complex_selector();

        // define an addition gate:
        meta.create_gate("add", |meta| {
            let w0 = meta.query_advice(w0, Rotation(0));
            let w1 = meta.query_advice(w1, Rotation(0));
            let w2 = meta.query_advice(w2, Rotation(0));
            let q_add = meta.query_selector(q_add);
            vec![q_add * (w0 + w1 - w2)]
        });

        // define a multiplication gate:
        meta.create_gate("mul", |meta| {
            let w0 = meta.query_advice(w0, Rotation(0));
            let w1 = meta.query_advice(w1, Rotation(0));
            let w2 = meta.query_advice(w2, Rotation(0));
            let q_mul = meta.query_selector(q_mul);
            vec![q_mul * (w0 * w1 - w2)]
        });

        Self {
            _ph: PhantomData,
            q_mul,
            q_add,
            w0,
            w1,
            w2,
        }
    }

    fn mul(
        &self,
        layouter: &mut impl Layouter<F>,
        lhs: AssignedCell<F, F>,
        rhs: AssignedCell<F, F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "mul",
            |mut region| {
                let w0 = lhs.value().cloned();
                let w1 = rhs.value().cloned();
                let w2 = w0.and_then(|w0| w1.and_then(|w1| Value::known(w0 * w1)));

                let w0 = region.assign_advice(|| "assign w0", self.w0, 0, || w0)?;
                let w1 = region.assign_advice(|| "assign w1", self.w1, 0, || w1)?;
                let w2 = region.assign_advice(|| "assign w2", self.w2, 0, || w2)?;
                self.q_mul.enable(&mut region, 0)?;

                region.constrain_equal(w0.cell(), lhs.cell())?;
                region.constrain_equal(w1.cell(), rhs.cell())?;

                Ok(w2)
            },
        )
    }

    fn add(
        &self,
        layouter: &mut impl Layouter<F>,
        lhs: AssignedCell<F, F>,
        rhs: AssignedCell<F, F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "add",
            |mut region| {
                let w0 = lhs.value().cloned();
                let w1 = rhs.value().cloned();
                let w2 = w0.and_then(|w0| w1.and_then(|w1| Value::known(w0 + w1)));

                let w0 = region.assign_advice(|| "assign w0", self.w0, 0, || w0)?;
                let w1 = region.assign_advice(|| "assign w1", self.w1, 0, || w1)?;
                let w2 = region.assign_advice(|| "assign w2", self.w2, 0, || w2)?;
                self.q_add.enable(&mut region, 0)?;

                region.constrain_equal(w0.cell(), lhs.cell())?;
                region.constrain_equal(w1.cell(), rhs.cell())?;

                Ok(w2)
            },
        )
    }

    fn free(
        &self,
        layouter: &mut impl Layouter<F>,
        value: Value<F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "free",
            |mut region| {
                let w0 = region.assign_advice(
                    //
                    || "assign w0",
                    self.w0,
                    0,
                    || value,
                )?;
                Ok(w0)
            },
        )
    }
}

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    arithmetic_chip: ArithmeticChip<F>,
    w0: Column<Advice>,
    w1: Column<Advice>,
    w2: Column<Advice>,
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
        let w0 = meta.advice_column();
        let w1 = meta.advice_column();
        let w2 = meta.advice_column();

        // enable equality constraints
        meta.enable_equality(w0);
        meta.enable_equality(w1);
        meta.enable_equality(w2);

        let arithmetic_chip = ArithmeticChip::configure(meta, w0, w1, w2);

        TestConfig {
            _ph: PhantomData,
            arithmetic_chip,
            w0,
            w1,
            w2,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let a1 = config
            .arithmetic_chip
            .free(&mut layouter, self.secret.clone())?;

        let a2 = config
            .arithmetic_chip
            .mul(&mut layouter, a1.clone(), a1.clone())?;

        let a3 = config
            .arithmetic_chip
            .add(&mut layouter, a1.clone(), a2.clone())?;

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
