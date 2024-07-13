use std::{
    collections::HashMap,
    marker::PhantomData,
    ops::{Add, Mul, Neg, Sub},
};

use halo2_proofs::{
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        Advice, Challenge, Circuit, Column, ConstraintSystem, Error, Expression, FirstPhase, Fixed,
        SecondPhase, Selector,
    },
    poly::Rotation,
};

use ff::{Field, PrimeField};

const DIM: usize = 9;
const SQR: usize = 3;

const SUDUKO: [[u8; DIM]; DIM] = [
    [5, 3, 0, 0, 7, 0, 0, 0, 0],
    [6, 0, 0, 1, 9, 5, 0, 0, 0],
    [0, 9, 8, 0, 0, 0, 0, 6, 0],
    [8, 0, 0, 0, 6, 0, 0, 0, 3],
    [4, 0, 0, 8, 0, 3, 0, 0, 1],
    [7, 0, 0, 0, 2, 0, 0, 0, 6],
    [0, 6, 0, 0, 0, 0, 2, 8, 0],
    [0, 0, 0, 4, 1, 9, 0, 0, 5],
    [0, 0, 0, 0, 8, 0, 0, 7, 9],
];

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    suduko: [[u8; DIM]; DIM],
    solutation: Value<[[u8; DIM]; DIM]>,
}

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

impl<F: Field> Neg for Variable<F> {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            mul: -self.mul,
            add: -self.add,
            val: self.val,
        }
    }
}

impl<F: Field> Sub<F> for Variable<F> {
    type Output = Self;

    fn sub(self, rhs: F) -> Self {
        Self {
            mul: self.mul,
            add: self.add - rhs,
            val: self.val,
        }
    }
}

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

#[derive(Clone, Debug)]
struct ChallengeChip<F: Field> {
    q_challenge: Selector,
    challenge: Challenge,
    w0: Column<Advice>,
    _ph: PhantomData<F>,
}

impl<F: Field> ChallengeChip<F> {
    fn configure(meta: &mut ConstraintSystem<F>, challenge: Challenge, w0: Column<Advice>) -> Self {
        let q_challenge = meta.selector();

        meta.create_gate("eq_challenge", |meta| {
            let w0 = meta.query_advice(w0, Rotation::cur());
            let chal = meta.query_challenge(challenge);
            let q_challenge = meta.query_selector(q_challenge);
            vec![q_challenge * (w0 - chal)]
        });

        Self {
            q_challenge,
            challenge,
            w0,
            _ph: PhantomData,
        }
    }

    fn challenge(&self, layouter: &mut impl Layouter<F>) -> Result<Variable<F>, Error> {
        let value = layouter.get_challenge(self.challenge);
        layouter.assign_region(
            || "challenge",
            |mut region| {
                self.q_challenge.enable(&mut region, 0)?;
                let val = region.assign_advice(|| "w0", self.w0, 0, || value)?;
                Ok(Variable {
                    mul: F::ONE,
                    add: F::ZERO,
                    val,
                })
            },
        )
    }
}

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
        lhs: &Variable<F>,
        rhs: &Variable<F>,
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
        lhs: &Variable<F>,
        rhs: &Variable<F>,
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

    fn sub(
        &self,
        layouter: &mut impl Layouter<F>,
        lhs: &Variable<F>,
        rhs: &Variable<F>,
    ) -> Result<Variable<F>, Error> {
        let minus = -rhs.clone();
        self.add(layouter, lhs, &minus)
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

    fn constant(&self, layouter: &mut impl Layouter<F>, constant: F) -> Result<Variable<F>, Error> {
        layouter.assign_region(
            || "constant",
            |mut region| {
                // turn on the arithmetic gate
                self.q_arith.enable(&mut region, 0)?;

                let val = region.assign_advice(|| "val", self.w0, 0, || Value::known(constant))?;
                region.assign_advice(|| "junk1", self.w1, 0, || Value::known(F::ZERO))?;
                region.assign_advice(|| "junk2", self.w2, 0, || Value::known(F::ZERO))?;

                region.assign_fixed(|| "c0", self.c0, 0, || Value::known(F::ONE))?;
                region.assign_fixed(|| "c1", self.c1, 0, || Value::known(F::ZERO))?;
                region.assign_fixed(|| "c2", self.c2, 0, || Value::known(F::ZERO))?;
                region.assign_fixed(|| "cc", self.cc, 0, || Value::known(-constant))?;
                region.assign_fixed(|| "cm", self.cm, 0, || Value::known(F::ZERO))?;

                Ok(Variable {
                    mul: F::ONE,
                    add: F::ZERO,
                    val,
                })
            },
        )
    }

    fn eq(
        &self,
        layouter: &mut impl Layouter<F>,
        lhs: &Variable<F>,
        rhs: &Variable<F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "eq",
            |mut region| {
                // turn on the arithmetic gate
                self.q_arith.enable(&mut region, 0)?;

                lhs.val.copy_advice(|| "lhs", &mut region, self.w0, 0)?;
                rhs.val.copy_advice(|| "rhs", &mut region, self.w1, 0)?;
                region.assign_advice(|| "junk2", self.w2, 0, || Value::known(F::ZERO))?;

                let delta = lhs.add - rhs.add;

                region.assign_fixed(|| "c0", self.c0, 0, || Value::known(lhs.mul))?;
                region.assign_fixed(|| "c1", self.c1, 0, || Value::known(-rhs.mul))?;
                region.assign_fixed(|| "c2", self.c2, 0, || Value::known(F::ZERO))?;
                region.assign_fixed(|| "cc", self.cc, 0, || Value::known(delta))?;
                region.assign_fixed(|| "cm", self.cm, 0, || Value::known(F::ZERO))?;

                Ok(())
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

    /// Allocate a bit-constrained variable.
    fn bit(
        &self,
        layouter: &mut impl Layouter<F>,
        value: Value<bool>,
    ) -> Result<Variable<F>, Error> {
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
    phase1_chip: ArithmeticChip<F>,
    phase2_chip: ArithmeticChip<F>,
    challenge_chip: ChallengeChip<F>,
}

impl<F: PrimeField> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            solutation: Value::unknown(),
            suduko: SUDUKO,
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

        let alpha = meta.challenge_usable_after(FirstPhase);

        let phase1_chip = ArithmeticChip::configure(meta, w0, w1, w2, c0, c1, c2, cc, cm);

        let w0_phase2 = meta.advice_column_in(SecondPhase);
        let w1_phase2 = meta.advice_column_in(SecondPhase);
        let w2_phase2 = meta.advice_column_in(SecondPhase);

        meta.enable_equality(w0_phase2);
        meta.enable_equality(w1_phase2);
        meta.enable_equality(w2_phase2);

        let phase2_chip =
            ArithmeticChip::configure(meta, w0_phase2, w1_phase2, w2_phase2, c0, c1, c2, cc, cm);

        let challenge_chip = ChallengeChip::configure(meta, alpha, w0_phase2);

        TestConfig {
            _ph: PhantomData,
            phase1_chip,
            phase2_chip,
            challenge_chip,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // load/fix the suduko
        let mut cells = vec![];
        for i in 0..DIM {
            let mut row = vec![];
            for j in 0..DIM {
                let cell = match self.suduko[i][j] {
                    0 => config.phase1_chip.free(
                        &mut layouter,
                        self.solutation.map(|sol| F::from_u128(sol[i][j] as u128)),
                    ),
                    fixed => config
                        .phase1_chip
                        .constant(&mut layouter, F::from_u128(fixed as u128)),
                }?;
                row.push(cell);
            }
            cells.push(row)
        }

        // distinct constraints
        let mut distinct = vec![];

        // row constraints
        for row in 0..DIM {
            distinct.push(
                cells[row]
                    .iter()
                    .map(|cell| cell.clone())
                    .collect::<Vec<_>>(),
            );
        }

        // column constraints
        for col in 0..DIM {
            distinct.push(cells.iter().map(|row| row[col].clone()).collect::<Vec<_>>());
        }

        // block constraints
        for i in 0..DIM / SQR {
            for j in 0..DIM / SQR {
                let row = i * SQR;
                let col = j * SQR;
                let mut block = vec![];
                for ii in 0..SQR {
                    for jj in 0..SQR {
                        block.push(cells[row + ii][col + jj].clone());
                    }
                }
                distinct.push(block);
            }
        }

        assert_eq!(distinct.len(), 9 + 9 + 9);

        // next phase
        let alpha = config.challenge_chip.challenge(&mut layouter)?;

        // allowed set of entries
        let mut numbers = vec![];
        for num in 1..=DIM {
            numbers.push(
                config
                    .phase2_chip
                    .constant(&mut layouter, F::from_u128(num as u128))?,
            );
        }

        // eval the vanish poly over the numbers
        let eval_known = eval_vanish(&mut layouter, &config.phase2_chip, &alpha, &numbers)?;

        // eval the vanish poly over the distinct cells and check against eval_known
        for dist in distinct.iter() {
            let eval_check = eval_vanish(&mut layouter, &config.phase2_chip, &alpha, &dist)?;
            config
                .phase2_chip
                .eq(&mut layouter, &eval_known, &eval_check)?;
        }

        Ok(())
    }
}

fn eval_vanish<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    chip: &ArithmeticChip<F>,
    alpha: &Variable<F>,
    terms: &[Variable<F>],
) -> Result<Variable<F>, Error> {
    let mut poly = chip.constant(layouter, F::ONE)?;
    for term in terms.iter() {
        let mono = chip.sub(layouter, term, alpha)?;
        poly = chip.mul(layouter, &poly, &mono)?;
    }
    Ok(poly)
}

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    const SOLUTION: [[u8; DIM]; DIM] = [
        [5, 3, 4, 6, 7, 8, 9, 1, 2],
        [6, 7, 2, 1, 9, 5, 3, 4, 8],
        [1, 9, 8, 3, 4, 2, 5, 6, 7],
        [8, 5, 9, 7, 6, 1, 4, 2, 3],
        [4, 2, 6, 8, 5, 3, 7, 9, 1],
        [7, 1, 3, 9, 2, 4, 8, 5, 6],
        [9, 6, 1, 5, 3, 7, 2, 8, 4],
        [2, 8, 7, 4, 1, 9, 6, 3, 5],
        [3, 4, 5, 2, 8, 6, 1, 7, 9],
    ];

    // check the solution
    for row in 0..DIM {
        for col in 0..DIM {
            if SUDUKO[row][col] != 0 {
                assert_eq!(SUDUKO[row][col], SOLUTION[row][col]);
            }
        }
    }

    for row in 0..DIM {
        let mut elems = HashMap::new();
        for col in 0..DIM {
            let elem = SOLUTION[row][col];
            assert!(!elems.contains_key(&elem));
            elems.insert(elem, ());
        }
    }

    for col in 0..DIM {
        let mut elems = HashMap::new();
        for row in 0..DIM {
            let elem = SOLUTION[row][col];
            assert!(!elems.contains_key(&elem));
            elems.insert(elem, ());
        }
    }

    // run the MockProver
    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        solutation: Value::known(SOLUTION),
        suduko: SUDUKO,
    };
    let prover = MockProver::run(14, &circuit, vec![]).unwrap();
    prover.verify().unwrap();
}
