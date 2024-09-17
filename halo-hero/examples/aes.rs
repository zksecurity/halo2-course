use std::{cell::RefCell, marker::PhantomData};

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
    transcript::{
        self, Blake2bRead, Blake2bWrite, Challenge255, PoseidonWrite, TranscriptReadBuffer,
    },
};

use ff::{Field, PrimeField};
use rand::rngs::ThreadRng;

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    a: Value<u8>, // secret
    b: Value<u8>, // secret
}

#[derive(Clone, Debug)]
struct FixedTableChip<F: PrimeField> {
    _ph: PhantomData<F>,
    off: RefCell<usize>,
    sel: Selector,
    typ: TableColumn,
    in1: TableColumn,
    in2: TableColumn,
    out: TableColumn,
}

impl<F: PrimeField> FixedTableChip<F> {
    fn configure(meta: &mut ConstraintSystem<F>) -> Self {
        let sel = meta.compress_selector();
        let typ = meta.fixed_column();

        let in1 = meta.lookup_table_column();
        let in2 = meta.lookup_table_column();
        let out = meta.lookup_table_column();

        meta.lookup("op", |meta| {
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

        Self {
            _ph: PhantomData,
            off: RefCell::new(0),
            sel,
            typ,
            in1,
            in2,
            out,
        }
    }

    fn append(
        &self,
        layouter: &mut impl Layouter<F>,
        config: &FixedTableChip<F>,
        typ: usize,
        elem: impl Iterator<Item = (F, F, F)>,
    ) -> Result<(), Error> {
        let off = self.off.borrow_mut();
        let nxt = *off;
        layouter.assign_table(
            || "xor-table",
            |mut table| {
                for (in1, in2, out) in elem {
                    table.assign_cell(|| "in1", config.in1, nxt, || Ok(in1))?;
                    table.assign_cell(|| "in2", config.in2, nxt, || Ok(in2))?;
                    table.assign_cell(|| "out", config.out, nxt, || Ok(out))?;
                    table.assign_cell(|| "typ", config.typ, nxt, || Ok(F::from_u64(typ as u64)))?;
                    nxt += 1;
                }
                Ok(())
            },
        )?;
        off = nxt;
        Ok(())
    }

    fn op_binary(
        &self,
        layouter: &mut impl Layouter<F>,
        config: &FixedTableChip<F>,
        typ: usize,
        elem: impl Iterator<Item = (F, F, F)>,
    ) {
        self.append(layouter, config, typ, elem)
    }

    fn op_unary(
        &self,
        layouter: &mut impl Layouter<F>,
        config: &FixedTableChip<F>,
        typ: usize,
        elem: impl Iterator<Item = (F, F)>,
    ) {
        self.append(
            layouter,
            config,
            typ,
            elem.map(|(in1, out)| (in1, F::zero(), out)),
        )
    }
}

#[derive(Clone, Debug)]
struct TestConfig<F: PrimeField> {
    _ph: PhantomData<F>,
    advice: Column<Advice>,
    fixed: Column<Fixed>,
    instance: Column<Instance>,
    fixed_tables: FixedTables<F>,
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

        let fixed_tables = FixedTableChip::configure(meta);

        let tbl_in1 = meta.lookup_table_column();
        let tbl_in2 = meta.lookup_table_column();
        let tbl_out = meta.lookup_table_column();

        // this will allow us to have equality constraints
        meta.enable_equality(advice);
        meta.enable_equality(instance);

        let arith = ArithmeticChip::configure(meta, advice, fixed);

        TestConfig {
            _ph: PhantomData,
            fixed,
            advice,
            instance,
            fixed_tables,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
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

    /*
    let vk_circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        a: Value::unknown(),
        b: Value::unknown(),
    };

    let mut rng = rand::thread_rng();
    use halo2_proofs::transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer};
        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);

        let srs = ParamsKZG::setup(8, &mut rng);
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
    */
}
