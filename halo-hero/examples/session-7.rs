use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    halo2curves::bn256::{Bn256, G1Affine},
    plonk::{
        self, create_proof, keygen_pk, keygen_vk, Advice, Challenge, Circuit, Column,
        ConstraintSystem, Error, Expression, FirstPhase, SecondPhase, Selector,
    },
    poly::{
        kzg::{
            commitment::{KZGCommitmentScheme, ParamsKZG},
            multiopen::ProverSHPLONK,
        },
        Rotation,
    },
};

use ff::Field;
use rand::rngs::ThreadRng;

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    eq_rows: Vec<(usize, usize)>,
    assignment: Value<Vec<[F; 3]>>,
}

#[derive(Clone, Debug)]
struct RLCChip<F: Field, const N: usize> {
    q_enable: Selector,
    advice: [Column<Advice>; N],
    challenge: Challenge,
    rlc: Column<Advice>, // rlc = (adv[0] + c * adv[1] + c^2 * adv[2] + ... + c^(N-1) * adv[N-1])
    _ph: PhantomData<F>,
}

impl<F: Field, const N: usize> RLCChip<F, N> {
    fn configure(meta: &mut ConstraintSystem<F>, advice: [Column<Advice>; N]) -> Self {
        let rlc = meta.advice_column_in(SecondPhase); // <- enforce equality on this
        let q_enable = meta.selector();
        let challenge = meta.challenge_usable_after(FirstPhase);

        meta.enable_equality(rlc);

        meta.create_gate("rlc", |meta| {
            let challenge = meta.query_challenge(challenge);

            let mut x = Expression::Constant(F::ONE);
            let mut y = Expression::Constant(F::ZERO);
            let rlc = meta.query_advice(rlc, Rotation::cur());
            let sel = meta.query_selector(q_enable);

            // intermediate_result (y) | value |
            //
            // y.next() = challenge * y.current() + value.current()
            //
            //
            //

            for adv in advice.iter() {
                y = y + meta.query_advice(adv.clone(), Rotation::cur()) * x.clone();
                x = x.clone() * challenge.clone();
            }
            vec![sel * (rlc - y)]
        });

        Self {
            q_enable,
            advice,
            challenge,
            rlc,
            _ph: PhantomData,
        }
    }

    fn compute_rlc(&self, challenge: F, advs: [F; N]) -> F {
        let mut rlc = F::ZERO;
        let mut c = F::ONE;
        for i in 0..N {
            rlc += advs[i] * c;
            c *= challenge;
        }
        rlc
    }

    fn alloc_row(
        &self,
        layouter: &mut impl Layouter<F>,
        value: Value<[F; N]>,
    ) -> Result<([AssignedCell<F, F>; N], AssignedCell<F, F>), Error> {
        let challenge = layouter.get_challenge(self.challenge);

        layouter.assign_region(
            || "fingerprint-row",
            |mut region| {
                let mut result = vec![];
                for i in 0..N {
                    result.push(region.assign_advice(
                        || format!("adv{}", i),
                        self.advice[i],
                        0,
                        || value.map(|v| v[i]),
                    )?);
                }

                self.q_enable.enable(&mut region, 0)?;

                let rlc = region.assign_advice(
                    || "rlc",
                    self.rlc,
                    0,
                    || {
                        challenge
                            .and_then(|c| value.and_then(|v| Value::known(self.compute_rlc(c, v))))
                    },
                )?;
                Ok((result.try_into().unwrap(), rlc))
            },
        )
    }
}

const ROW_EQUALITY: [(usize, usize); 3] = [
    (0, 3), //
    (0, 5),
    (1, 2),
];

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    rlc_chip: RLCChip<F, 3>,
    advs: [Column<Advice>; 3],
}

impl<F: Field> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            eq_rows: ROW_EQUALITY.to_vec(),
            assignment: Value::unknown(),
        }
    }

    #[allow(unused_variables)]
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advs = [
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
        ];

        let rlc_chip = RLCChip::configure(meta, advs);

        TestConfig {
            _ph: PhantomData,
            rlc_chip,
            advs,
        }
    }

    #[allow(unused_variables)]
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), plonk::Error> {
        println!("synthesizing");

        // compute the number of rows
        let num_rows = self
            .eq_rows
            .iter()
            .map(|(a, b)| std::cmp::max(a, b))
            .max()
            .unwrap()
            + 1;

        // allocate the rows
        let mut rows = vec![];
        let mut rlcs = vec![];
        for i in 0..num_rows {
            let (row, rlc) = config
                .rlc_chip
                .alloc_row(&mut layouter, self.assignment.as_ref().map(|v| v[i]))?;
            rows.push(row);
            rlcs.push(rlc);
        }

        //
        layouter.assign_region(
            || "eq",
            |mut region| {
                for (lhs, rhs) in self.eq_rows.iter().copied() {
                    region.constrain_equal(rlcs[lhs].cell(), rlcs[rhs].cell())?;
                }
                Ok(())
            },
        )?;

        Ok(())
    }
}

fn main() {
    use ff::PrimeField;
    use halo2_proofs::halo2curves::bn256::Fr;
    let assignment = [
        [Fr::from_u128(1), Fr::from_u128(2), Fr::from_u128(3)], // row 0
        [Fr::from_u128(4), Fr::from_u128(5), Fr::from_u128(6)], // row 1
        [Fr::from_u128(4), Fr::from_u128(5), Fr::from_u128(6)], // row 2
        [Fr::from_u128(1), Fr::from_u128(2), Fr::from_u128(3)], // row 3
        [
            Fr::from_u128(0xbeef),
            Fr::from_u128(0xcafe),
            Fr::from_u128(0xf00d),
        ], // row 4
        [Fr::from_u128(1), Fr::from_u128(2), Fr::from_u128(3)], // row 5
    ];

    println!("check witness");
    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        eq_rows: ROW_EQUALITY.to_vec(),
        assignment: Value::known(assignment.to_vec()),
    };
    let prover = MockProver::run(8, &circuit, vec![]).unwrap();
    prover.verify().unwrap();

    let mut rng = rand::thread_rng();
    use halo2_proofs::transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer};
    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);

    println!("compute vk/pk");

    let srs = ParamsKZG::setup(8, &mut rng);
    let vk = keygen_vk(&srs, &circuit).unwrap(); // public
    let pk = keygen_pk(&srs, vk.clone(), &circuit).unwrap();

    println!("creating proof:");
    println!("this should run synthesis twice: once for each phase");

    create_proof::<
        KZGCommitmentScheme<Bn256>,
        ProverSHPLONK<'_, Bn256>,
        Challenge255<G1Affine>,
        ThreadRng,
        Blake2bWrite<Vec<u8>, G1Affine, Challenge255<G1Affine>>,
        TestCircuit<Fr>,
    >(&srs, &pk, &[circuit], &[&[]], rng, &mut transcript)
    .unwrap();
}
