use std::{
    marker::PhantomData,
    ops::{Add, Mul},
};

use halo2_proofs::{
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        Advice,
        Circuit,
        Column,
        ConstraintSystem, //
        Error,
        Expression,
        Fixed,
        Selector,
    },
    poly::Rotation,
};

use ff::{Field, PrimeField};

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    secret: Value<F>,
}

// ANCHOR: variable
#[derive(Clone, Debug)]
struct Variable<F: Field> {
    mul: F,
    add: F,
    val: AssignedCell<F, F>,
}

impl<F: Field> Variable<F> {
    fn value(&self) -> Value<F> {
        self.val.value().map(|v| self.mul * v + self.add)
    }
}
// ANCHOR_END: variable

// ANCHOR: add-mul-const
impl<F: Field> Add<F> for Variable<F> {
    type Output = Self;

    fn add(self, rhs: F) -> Self {
        Self {
            mul: self.mul,
            add: self.add + rhs,
            val: self.val,
        }
    }
}

impl<F: Field> Mul<F> for Variable<F> {
    type Output = Self;

    fn mul(self, rhs: F) -> Self {
        Self {
            mul: self.mul * rhs,
            add: self.add * rhs,
            val: self.val,
        }
    }
}
// ANCHOR_END: add-mul-const

#[derive(Clone, Debug)]
struct ArithmeticChip<F: Field> {
    _ph: PhantomData<F>,
    q_arith: Selector,
    cm: Column<Fixed>,
    c0: Column<Fixed>,
    c1: Column<Fixed>,
    c2: Column<Fixed>,
    cc: Column<Fixed>,
    w0: Column<Advice>,
    w1: Column<Advice>,
    w2: Column<Advice>,
}

impl<F: Field> ArithmeticChip<F> {
    fn configure(
        meta: &mut ConstraintSystem<F>,
        w0: Column<Advice>,
        w1: Column<Advice>,
        w2: Column<Advice>,
        c0: Column<Fixed>,
        c1: Column<Fixed>,
        c2: Column<Fixed>,
        cm: Column<Fixed>,
        cc: Column<Fixed>,
    ) -> Self {
        let q_arith = meta.complex_selector();

        // define arithmetic gate
        meta.create_gate("arith", |meta| {
            let w0 = meta.query_advice(w0, Rotation::cur());
            let w1 = meta.query_advice(w1, Rotation::cur());
            let w2 = meta.query_advice(w2, Rotation::cur());

            let c0 = meta.query_fixed(c0, Rotation::cur());
            let c1 = meta.query_fixed(c1, Rotation::cur());
            let c2 = meta.query_fixed(c2, Rotation::cur());

            let cm = meta.query_fixed(cm, Rotation::cur());
            let cc = meta.query_fixed(cc, Rotation::cur());

            let q_arith = meta.query_selector(q_arith);

            // define the arithmetic expression
            //
            // w0 * c0 + w1 * c1 + w2 * c2 + cm * (w0 * w1) + cc
            let expr = Expression::Constant(F::ZERO);
            let expr = expr + c0 * w0.clone();
            let expr = expr + c1 * w1.clone();
            let expr = expr + c2 * w2.clone();
            let expr = expr + cm * (w0 * w1);
            let expr = expr + cc;
            vec![q_arith * expr]
        });

        Self {
            _ph: PhantomData,
            q_arith,
            cm,
            c0,
            c1,
            c2,
            cc,
            w0,
            w1,
            w2,
        }
    }

    /// Multiply two variables
    fn mul(
        &self,
        layouter: &mut impl Layouter<F>,
        lhs: Variable<F>,
        rhs: Variable<F>,
    ) -> Result<Variable<F>, Error> {
        layouter.assign_region(
            || "mul",
            |mut region| {
                // turn on the arithmetic gate
                self.q_arith.enable(&mut region, 0)?;

                // (c0 * w0 + cc1) * (c1 * w1 + cc2)
                // c0 * c1 * (w0 * w1) + c0 * cc2 * w0 + c1 * cc1 * w1 + cc1 * cc2
                lhs.val.copy_advice(|| "lhs", &mut region, self.w0, 0)?;
                rhs.val.copy_advice(|| "rhs", &mut region, self.w1, 0)?;

                let val =
                    region.assign_advice(|| "res", self.w2, 0, || lhs.value() * rhs.value())?;

                region.assign_fixed(|| "c0", self.c0, 0, || Value::known(lhs.mul * rhs.add))?;
                region.assign_fixed(|| "c1", self.c1, 0, || Value::known(rhs.mul * lhs.add))?;
                region.assign_fixed(|| "c2", self.c2, 0, || Value::known(-F::ONE))?;
                region.assign_fixed(|| "cc", self.cc, 0, || Value::known(lhs.add * rhs.add))?;
                region.assign_fixed(|| "cm", self.cm, 0, || Value::known(lhs.mul * rhs.mul))?;

                Ok(Variable {
                    mul: F::ONE,
                    add: F::ZERO,
                    val,
                })
            },
        )
    }

    /// Add two variables
    fn add(
        &self,
        layouter: &mut impl Layouter<F>,
        lhs: Variable<F>,
        rhs: Variable<F>,
    ) -> Result<Variable<F>, Error> {
        layouter.assign_region(
            || "add",
            |mut region| {
                // turn on the arithmetic gate
                self.q_arith.enable(&mut region, 0)?;

                lhs.val.copy_advice(|| "lhs", &mut region, self.w0, 0)?;
                rhs.val.copy_advice(|| "rhs", &mut region, self.w1, 0)?;

                let val =
                    region.assign_advice(|| "res", self.w2, 0, || lhs.value() + rhs.value())?;

                region.assign_fixed(|| "c0", self.c0, 0, || Value::known(lhs.mul))?;
                region.assign_fixed(|| "c1", self.c1, 0, || Value::known(rhs.mul))?;
                region.assign_fixed(|| "c2", self.c2, 0, || Value::known(-F::ONE))?;
                region.assign_fixed(|| "cc", self.cc, 0, || Value::known(lhs.add + rhs.add))?;
                region.assign_fixed(|| "cm", self.cm, 0, || Value::known(F::ZERO))?;

                Ok(Variable {
                    mul: F::ONE,
                    add: F::ZERO,
                    val,
                })
            },
        )
    }

    /// Allocate a free variable.
    fn free(&self, layouter: &mut impl Layouter<F>, value: Value<F>) -> Result<Variable<F>, Error> {
        layouter.assign_region(
            || "free",
            |mut region| {
                // no need to turn on anything
                let val = region.assign_advice(|| "free", self.w0, 0, || value)?;
                region.assign_advice(|| "junk1", self.w1, 0, || Value::known(F::ZERO))?;
                region.assign_advice(|| "junk2", self.w2, 0, || Value::known(F::ZERO))?;
                Ok(Variable {
                    mul: F::ONE,
                    add: F::ZERO,
                    val,
                })
            },
        )
    }

    /// Assert equal
    fn eq_consant(
        &self,
        layouter: &mut impl Layouter<F>,
        constant: F,
        variable: Variable<F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "eq_constant",
            |mut region| {
                // turn on the arithmetic gate
                self.q_arith.enable(&mut region, 0)?;

                variable
                    .val
                    .copy_advice(|| "val", &mut region, self.w0, 0)?;

                let delta = variable.add - constant;

                region.assign_advice(|| "junk1", self.w1, 0, || Value::known(F::ZERO))?;
                region.assign_advice(|| "junk2", self.w2, 0, || Value::known(F::ZERO))?;

                region.assign_fixed(|| "c0", self.c0, 0, || Value::known(variable.mul))?;
                region.assign_fixed(|| "c1", self.c1, 0, || Value::known(F::ZERO))?;
                region.assign_fixed(|| "c2", self.c2, 0, || Value::known(F::ZERO))?;
                region.assign_fixed(|| "cc", self.cc, 0, || Value::known(delta))?;
                region.assign_fixed(|| "cm", self.cm, 0, || Value::known(F::ZERO))?;

                Ok(())
            },
        )
    }

    // ANCHOR: bit
    /// Allocate a bit-constrained variable.
    fn bit(
        &self,
        layouter: &mut impl Layouter<F>,
        value: Value<bool>,
    ) -> Result<Variable<F>, Error> {
        // ANCHOR_END: bit
        layouter.assign_region(
            || "bit",
            |mut region| {
                // turn on the arithmetic gate
                self.q_arith.enable(&mut region, 0)?;

                // (v1 - 1) * v1 = v1^2 - v1
                let w0 = region.assign_advice(
                    || "bit0",
                    self.w0,
                    0,
                    || value.map(|b| if b { F::ONE } else { F::ZERO }),
                )?;

                let w1 = region.assign_advice(
                    || "bit1",
                    self.w1,
                    0,
                    || value.map(|b| if b { F::ONE } else { F::ZERO }),
                )?;

                region.assign_advice(|| "junk", self.w2, 0, || Value::known(F::ZERO))?;

                region.constrain_equal(w0.cell(), w1.cell())?;

                region.assign_fixed(|| "c0", self.c0, 0, || Value::known(F::ZERO))?;
                region.assign_fixed(|| "c1", self.c0, 0, || Value::known(-F::ONE))?;
                region.assign_fixed(|| "c2", self.c0, 0, || Value::known(F::ZERO))?;
                region.assign_fixed(|| "cc", self.cc, 0, || Value::known(F::ZERO))?;
                region.assign_fixed(|| "cm", self.cm, 0, || Value::known(F::ONE))?;

                Ok(Variable {
                    mul: F::ONE,
                    add: F::ZERO,
                    val: w0,
                })
            },
        )
    }
}

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    arithmetic_chip: ArithmeticChip<F>,
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
        // let q_enable = meta.fixed_column();
        let w0 = meta.advice_column();
        let w1 = meta.advice_column();
        let w2 = meta.advice_column();

        let c0 = meta.fixed_column();
        let c1 = meta.fixed_column();
        let c2 = meta.fixed_column();
        let cc = meta.fixed_column();
        let cm = meta.fixed_column();

        // enable equality constraints
        meta.enable_equality(w0);
        meta.enable_equality(w1);
        meta.enable_equality(w2);

        let arithmetic_chip = ArithmeticChip::configure(meta, w0, w1, w2, c0, c1, c2, cc, cm);

        TestConfig {
            _ph: PhantomData,
            arithmetic_chip,
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
            .add(&mut layouter, a1.clone(), a1.clone())?;

        let a3 = config
            .arithmetic_chip
            .mul(&mut layouter, a1.clone(), a2.clone())?;

        config
            .arithmetic_chip
            .eq_consant(&mut layouter, F::from_u128(1337 * (1337 + 1337)), a3)?;

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
