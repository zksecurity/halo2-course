use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{layouter, AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    halo2curves::bn256::{Bn256, G1Affine},
    plonk::{
        create_proof, keygen_pk, keygen_vk, verify_proof, Advice, Circuit, Column,
        ConstraintSystem, Error, Expression, Fixed, Instance, Selector, TableColumn,
    },
    poly::{
        commitment::Prover,
        kzg::{
            commitment::{KZGCommitmentScheme, ParamsKZG},
            multiopen::{ProverSHPLONK, VerifierSHPLONK},
            strategy::SingleStrategy,
        },
        Rotation, VerificationStrategy,
    },
    transcript::{self, Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer},
};

use ff::{Field, PrimeField};
use rand::rngs::ThreadRng;

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    a: Value<u8>, // secret
    b: Value<u8>, // secret
}

#[derive(Clone, Debug)]
struct TestConfig<F: PrimeField> {
    _ph: PhantomData<F>,
    advice: Column<Advice>,
    fixed: Column<Fixed>,
    instance: Column<Instance>,

    // XOR table:
    // | a | b | a ^ b |
    // for a in 0..16:
    //    for b in 0..16:
    //        tbl[i] = (a, b, a ^ b)
    //
    // Check:
    //
    // (lhs, rhs, out) in tbl
    //
    // Implies:
    //
    // lhs in [0, 16)
    // rhs in [0, 16)
    // out = lhs ^ rhs
    tbl_in1: TableColumn,
    tbl_in2: TableColumn,
    tbl_out: TableColumn,
    q_xor: Selector,
    arith: ArithmeticChip<F>,
}

#[derive(Debug, Clone)]
struct ArithmeticChip<F: PrimeField> {
    q_add: Selector,
    q_mul: Selector,
    q_fix: Selector,
    advice: Column<Advice>,
    fixed: Column<Fixed>,
    _ph: PhantomData<F>,
}

impl<F: PrimeField> ArithmeticChip<F> {
    // allocate a new unconstrained value
    fn free(
        &self,
        layouter: &mut impl Layouter<F>,
        value: Value<F>, // this something prover knowns
    ) -> Result<AssignedCell<F, F>, Error> {
        // the region is going to have a height of 1
        // | Advice (advice) | Selector (q_mul) |
        // |              w0 |                0 |
        layouter.assign_region(
            || "free",
            |mut region| {
                let w0 = region.assign_advice(|| "w0", self.advice, 0, || value)?;
                Ok(w0)
            },
        )
    }

    // input = constant
    fn fixed(
        &self,
        layouter: &mut impl Layouter<F>,
        input: AssignedCell<F, F>,
        constant: F, // verifier (fixed in circuit)
    ) -> Result<(), Error> {
        // | Advice (advice) | Selector (q_fix) | Fixed (fixed) |
        // |             w0  |                1 |            c0 |
        layouter.assign_region(
            || "fixed",
            |mut region| {
                let w0 =
                    region.assign_advice(|| "w0", self.advice, 0, || input.value().cloned())?;
                let c0 = region.assign_fixed(|| "c0", self.fixed, 0, || Value::known(constant))?;

                // force input = w0
                region.constrain_equal(w0.cell(), input.cell())?;
                self.q_fix.enable(&mut region, 0)?;
                Ok(())
            },
        )
    }

    // helper function to generate multiplication gates
    fn mul(
        &self,
        layouter: &mut impl Layouter<F>,
        lhs: AssignedCell<F, F>,
        rhs: AssignedCell<F, F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        // the region is going to have a height of 3
        // | Advice (advice) | Selector (q_mul) |
        // |              w0 |                1 |
        // |              w1 |                0 |
        // |              w2 |                0 |
        layouter.assign_region(
            || "mul",
            |mut region| {
                // turn on the gate
                self.q_mul.enable(&mut region, 0)?;

                // assign the witness value to the advice column
                let w0 = region.assign_advice(|| "w0", self.advice, 0, || lhs.value().cloned())?;

                let w1 = region.assign_advice(|| "w1", self.advice, 1, || rhs.value().cloned())?;

                let w2 = region.assign_advice(
                    || "w2",
                    self.advice,
                    2,
                    || lhs.value().cloned() * rhs.value().cloned(),
                )?;

                region.constrain_equal(w0.cell(), lhs.cell())?;
                region.constrain_equal(w1.cell(), rhs.cell())?;

                Ok(w2)
            },
        )
    }

    // helper function to generate multiplication gates
    fn add(
        &self,
        layouter: &mut impl Layouter<F>,
        lhs: AssignedCell<F, F>,
        rhs: AssignedCell<F, F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        // the region is going to have a height of 3
        // | Advice (advice) | Selector (q_mul) |
        // |              w0 |                1 |
        // |              w1 |                0 |
        // |              w2 |                0 |
        layouter.assign_region(
            || "add",
            |mut region| {
                // turn on the gate
                self.q_add.enable(&mut region, 0)?;

                // assign the witness value to the advice column
                let w0 = region.assign_advice(|| "w0", self.advice, 0, || lhs.value().cloned())?;

                let w1 = region.assign_advice(|| "w1", self.advice, 1, || rhs.value().cloned())?;

                let w2 = region.assign_advice(
                    || "w2",
                    self.advice,
                    2,
                    || lhs.value().cloned() + rhs.value().cloned(),
                )?;

                region.constrain_equal(w0.cell(), lhs.cell())?;
                region.constrain_equal(w1.cell(), rhs.cell())?;

                Ok(w2)
            },
        )
    }

    fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: Column<Advice>,
        fixed: Column<Fixed>,
    ) -> Self {
        let q_mul = meta.complex_selector();
        let q_add = meta.complex_selector();
        let q_fix = meta.complex_selector();

        // if q_fix = 1: c0 = w0
        meta.create_gate("fixed", |meta| {
            let w0 = meta.query_advice(advice, Rotation::cur());
            let c0 = meta.query_fixed(fixed, Rotation::cur());
            let q_fix = meta.query_selector(q_fix);
            vec![q_fix * (w0 - c0)]
        });

        // define a new gate:
        // next = curr + 1 if q_enable is 1
        meta.create_gate("vertical-mul", |meta| {
            //            | Advice |
            // current -> |     w0 |
            //            |     w1 |
            //            |     w2 |
            let w0 = meta.query_advice(advice, Rotation::cur()); // current row
            let w1 = meta.query_advice(advice, Rotation::next()); // next row
            let w2 = meta.query_advice(advice, Rotation(2)); // next next row

            let q_mul = meta.query_selector(q_mul);

            // w2 = w1 * w0 <-- sat.
            vec![q_mul * (w1 * w0 - w2)]
        });

        // define a new gate:
        // next = curr + 1 if q_enable is 1
        meta.create_gate("vertical-add", |meta| {
            //            | Advice |
            // current -> |     w0 |
            //            |     w1 |
            //            |     w2 |
            let w0 = meta.query_advice(advice, Rotation::cur()); // current row
            let w1 = meta.query_advice(advice, Rotation::next()); // next row
            let w2 = meta.query_advice(advice, Rotation(2)); // next next row

            let q_add = meta.query_selector(q_add);

            // w2 = w1 * w0 <-- sat.
            vec![q_add * (w1 + w0 - w2)]
        });

        ArithmeticChip {
            q_add,
            q_mul,
            q_fix,
            advice,
            fixed,
            _ph: PhantomData,
        }
    }
}

struct Bit4Ranged<F: PrimeField> {
    var: AssignedCell<F, F>,
    val: Value<u8>,
}

struct Bit8Ranged<F: PrimeField> {
    low: Bit4Ranged<F>,
    high: Bit4Ranged<F>,
}

impl<F: PrimeField> TestCircuit<F> {
    fn bits(
        config: &TestConfig<F>,
        layouter: &mut impl Layouter<F>,
        val: Value<u8>,
    ) -> Result<Bit4Ranged<F>, Error> {
        let var = config
            .arith
            .free(layouter, val.map(|v| F::from_u128(v as u128)))?;
        Ok(Bit4Ranged { var, val })
    }

    fn xor(
        config: &TestConfig<F>,
        layouter: &mut impl Layouter<F>,
        lhs: Bit4Ranged<F>,
        rhs: Bit4Ranged<F>,
    ) -> Result<Bit4Ranged<F>, Error> {
        layouter.assign_region(
            || "xor-region",
            |mut region| {
                // turn on the xor gate
                config.q_xor.enable(&mut region, 0)?;

                // remember: also enforces equality between lhs/rhs and w0/w1
                let w0 = lhs
                    .var
                    .copy_advice(|| "w0", &mut region, config.advice, 0)?;
                let w1 = rhs
                    .var
                    .copy_advice(|| "w1", &mut region, config.advice, 1)?;

                let val = lhs
                    .val
                    .and_then(|in1| rhs.val.and_then(|in2| Value::known(in1 ^ in2)));

                let w2 = region.assign_advice(
                    || "w2",
                    config.advice,
                    2,
                    || val.map(|v| F::from_u128(v as u128)),
                )?;

                Ok(Bit4Ranged { var: w2, val })
            },
        )
    }
}

impl<F: PrimeField> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            a: Value::unknown(),
            b: Value::unknown(),
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = meta.advice_column();
        let fixed = meta.fixed_column();
        let instance = meta.instance_column();

        let q_xor = meta.complex_selector();

        let tbl_in1 = meta.lookup_table_column();
        let tbl_in2 = meta.lookup_table_column();
        let tbl_out = meta.lookup_table_column();

        meta.lookup("xor", |meta| {
            let w0 = meta.query_advice(advice, Rotation(0)); // current row
            let w1 = meta.query_advice(advice, Rotation(1)); // next row
            let w2 = meta.query_advice(advice, Rotation(2)); // next next row
            let q_xor = meta.query_selector(q_xor);
            vec![
                (q_xor.clone() * w0, tbl_in1),
                (q_xor.clone() * w1, tbl_in2),
                (q_xor.clone() * w2, tbl_out),
            ]
        });

        // this will allow us to have equality constraints
        meta.enable_equality(advice);
        meta.enable_equality(instance);

        let arith = ArithmeticChip::configure(meta, advice, fixed);

        TestConfig {
            _ph: PhantomData,
            fixed,
            advice,
            instance,
            tbl_in1,
            tbl_in2,
            tbl_out,
            q_xor,

            arith,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // fill in the fixed table
        layouter.assign_table(
            || "xor-table",
            |mut table| {
                let mut row = 0;
                for in1 in 0..16 {
                    for in2 in 0..16 {
                        table.assign_cell(
                            || "in1",
                            config.tbl_in1,
                            row,
                            || Value::known(F::from_u128(in1)),
                        )?;
                        table.assign_cell(
                            || "in2",
                            config.tbl_in2,
                            row,
                            || Value::known(F::from_u128(in2)),
                        )?;
                        table.assign_cell(
                            || "out",
                            config.tbl_out,
                            row,
                            || Value::known(F::from_u128(in1 ^ in2)),
                        )?;
                        row += 1;
                    }
                }
                Ok(())
            },
        )?;

        // asking the prover to provide:
        // - a
        // - b
        let a = Self::bits(&config, &mut layouter, self.a)?;
        let b = Self::bits(&config, &mut layouter, self.b)?;

        // c = a * b
        let c = Self::xor(&config, &mut layouter, a, b)?;

        // instance[0] = c
        layouter.constrain_instance(c.var.cell(), config.instance, 0)?;
        Ok(())
    }
}

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    let k = 9;

    // run the MockProver
    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        a: Value::known(0xe),
        b: Value::known(0xb),
    };

    let instances = vec![Fr::from_u128(0x5 as u128)];

    let prover = MockProver::run(k, &circuit, vec![instances.clone()]).unwrap();
    prover.verify().unwrap();

    println!("create proof");

    let vk_circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        a: Value::unknown(),
        b: Value::unknown(),
    };

    let mut rng = rand::thread_rng();
    use halo2_proofs::transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer};
    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);

    let srs = ParamsKZG::setup(k, &mut rng);
    let vk = keygen_vk(&srs, &vk_circuit).unwrap(); // public
    let pk = keygen_pk(&srs, vk.clone(), &circuit).unwrap();

    create_proof::<
        KZGCommitmentScheme<Bn256>,
        ProverSHPLONK<'_, Bn256>,
        Challenge255<G1Affine>,
        ThreadRng,
        Blake2bWrite<Vec<u8>, G1Affine, Challenge255<G1Affine>>,
        TestCircuit<Fr>,
    >(
        &srs,
        &pk,
        &[circuit],
        &[&[&instances]],
        rng,
        &mut transcript,
    )
    .unwrap();

    let pf: Vec<u8> = transcript.finalize(); // public

    println!("proof-size: {:?}", pf.len());

    let mut transcript = Blake2bRead::init(&pf[..]);

    verify_proof::<
        KZGCommitmentScheme<Bn256>,
        VerifierSHPLONK<'_, Bn256>,
        Challenge255<G1Affine>,
        Blake2bRead<&[u8], G1Affine, Challenge255<G1Affine>>,
        SingleStrategy<'_, Bn256>,
    >(
        &srs,
        &vk,
        SingleStrategy::new(&srs),
        &[&[&instances]],
        &mut transcript,
    )
    .unwrap();
}
