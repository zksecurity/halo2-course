use std::{
    marker::PhantomData,
    ops::{Add, Mul, Neg, Sub},
};

use halo2_proofs::{
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        Advice,
        Challenge,
        Circuit,
        Column, //
        ConstraintSystem,
        Error,
        Expression,
        FirstPhase,
        Fixed,
        SecondPhase,
        Selector,
    },
    poly::Rotation,
};

use ff::{Field, PrimeField};

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
}

// ANCHOR: challenge_chip
#[derive(Clone, Debug)]
struct ChallengeChip<F: Field> {
    q_enable: Selector,
    challenge: Challenge,
    advice: Column<Advice>,
    _ph: PhantomData<F>,
}

impl<F: Field> ChallengeChip<F> {
    fn configure(
        meta: &mut ConstraintSystem<F>, //
        challenge: Challenge,
        w0: Column<Advice>,
    ) -> Self {
        let q_challenge = meta.selector();

        meta.create_gate("eq_challenge", |meta| {
            let w0 = meta.query_advice(w0, Rotation::cur());
            let chal = meta.query_challenge(challenge);
            let q_challenge = meta.query_selector(q_challenge);
            vec![q_challenge * (w0 - chal)]
        });

        Self {
            q_enable: q_challenge,
            challenge,
            advice: w0,
            _ph: PhantomData,
        }
    }
}
// ANCHOR_END: challenge_chip

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    challenge_chip: ChallengeChip<F>,
}

impl<F: PrimeField> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit { _ph: PhantomData }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        // let q_enable = meta.fixed_column();
        let w0 = meta.advice_column();
        let w1 = meta.advice_column();
        let w2 = meta.advice_column();

        // enable equality constraints
        meta.enable_equality(w0);
        meta.enable_equality(w1);
        meta.enable_equality(w2);

        let alpha = meta.challenge_usable_after(FirstPhase);

        let w0_phase2 = meta.advice_column_in(SecondPhase);
        let w1_phase2 = meta.advice_column_in(SecondPhase);
        let w2_phase2 = meta.advice_column_in(SecondPhase);

        meta.enable_equality(w0_phase2);
        meta.enable_equality(w1_phase2);
        meta.enable_equality(w2_phase2);

        TestConfig {
            challenge_chip: ChallengeChip::configure(meta, alpha, w0),
            _ph: PhantomData,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    // run the MockProver
    let circuit = TestCircuit::<Fr> { _ph: PhantomData };
    let prover = MockProver::run(10, &circuit, vec![]).unwrap();
    prover.verify().unwrap();
}
