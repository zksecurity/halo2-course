use std::{cell::RefCell, marker::PhantomData};

use halo2_proofs::{
    circuit::{AssignedCell, Layouter, Region, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        self, Advice, Circuit, Column, ConstraintSystem, Expression, Fixed, Selector, VirtualCells,
    },
    poly::Rotation,
};
use rand_chacha::ChaCha8Rng;

use ff::Field;
use rand::SeedableRng;

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
}

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    poseidon: PoseidonChip<F>,
    free: Column<Advice>,
}

// ANCHOR: poseidon_params
const ROUNDS: usize = 8;
const WIDTH: usize = 3;

const CAPACITY: usize = 1;
const RATE: usize = WIDTH - CAPACITY;
// ANCHOR_END: poseidon_params

const MAX_OPS_POSEIDON: usize = 10;

// ensure that POWER does not divide (r - 1)
// (otherwise it is not a permutation)
const POWER: u64 = 5;

// ANCHOR: poseidon_table
// a simplified version of Poseidon permutation
#[derive(Debug, Clone)]
struct PoseidonTable<F: Field + Clone> {
    matrix: [[F; WIDTH]; WIDTH],
    round_constants: [[F; WIDTH]; ROUNDS],
    flag_start: Column<Fixed>, // start of permutation
    flag_round: Column<Fixed>, // apply round
    flag_final: Column<Fixed>, // end of permutation
    inp1: Column<Advice>,
    inp2: Column<Advice>,
    rndc: [Column<Fixed>; WIDTH],
    cols: [Column<Advice>; WIDTH],
    _ph: PhantomData<F>,
}
// ANCHOR_END: poseidon_table

// Cauchy matrix
fn poseidon_matrix<F: Field>() -> [[F; WIDTH]; WIDTH] {
    let mut matrix = [[F::ZERO; WIDTH]; WIDTH];
    let mut rng = ChaCha8Rng::seed_from_u64(0x8badf00d);
    let xi = [
        F::random(&mut rng),
        F::random(&mut rng),
        F::random(&mut rng),
    ];
    let yi = [
        F::random(&mut rng),
        F::random(&mut rng),
        F::random(&mut rng),
    ];
    for i in 0..WIDTH {
        for j in 0..WIDTH {
            matrix[i][j] = (xi[i] + yi[j]).invert().unwrap();
        }
    }
    matrix
}

fn poseidon_round_constants<F: Field>() -> [[F; WIDTH]; ROUNDS] {
    let mut round_constants = [[F::ZERO; WIDTH]; ROUNDS];
    let mut rng = ChaCha8Rng::seed_from_u64(0xdeadc0de);
    for i in 0..ROUNDS {
        for j in 0..WIDTH {
            round_constants[i][j] = F::random(&mut rng);
        }
    }
    round_constants
}

fn poseidon_round<F: Field>(
    mat: &[[F; WIDTH]; WIDTH],
    rc: &[F; WIDTH],
    st: [F; WIDTH],
) -> [F; WIDTH] {
    fn sbox<F: Field>(x: F) -> F {
        x * x * x * x * x
    }

    let st = [
        st[0] + rc[0], //
        st[1] + rc[1],
        st[2] + rc[2],
    ];

    let st = [
        sbox(st[0]), //
        sbox(st[1]),
        sbox(st[2]),
    ];

    let st = [
        mat[0][0] * st[0] + mat[0][1] * st[1] + mat[0][2] * st[2], //
        mat[1][0] * st[0] + mat[1][1] * st[1] + mat[1][2] * st[2],
        mat[2][0] * st[0] + mat[2][1] * st[1] + mat[2][2] * st[2],
    ];

    st
}

struct PoseidonExprs<F: Field> {
    pub flag: Expression<F>,
    pub inp1: Expression<F>,
    pub inp2: Expression<F>,
    pub out: Expression<F>,
}

impl<F: Field> PoseidonTable<F> {
    // ANCHOR: poseidon_table_expr
    fn table_expr(&self, meta: &mut VirtualCells<F>) -> PoseidonExprs<F> {
        PoseidonExprs {
            flag: meta.query_any(self.flag_final, Rotation::cur()),
            inp1: meta.query_any(self.inp1, Rotation::cur()),
            inp2: meta.query_any(self.inp2, Rotation::cur()),
            out: meta.query_any(self.cols[0], Rotation::cur()),
        }
    }
    // ANCHOR_END: poseidon_table_expr

    fn hash(&self, in1: F, in2: F) -> F {
        let mut state = [in1, in2, F::ZERO];
        for r in 0..ROUNDS {
            state = poseidon_round(&self.matrix, &self.round_constants[r], state);
        }
        state[0]
    }

    fn new(meta: &mut ConstraintSystem<F>) -> Self {
        let matrix = poseidon_matrix();
        let round_constants = poseidon_round_constants();

        let cols = [
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
        ];

        let rndc = [
            meta.fixed_column(),
            meta.fixed_column(),
            meta.fixed_column(),
        ];

        let inp1 = meta.advice_column();
        let inp2 = meta.advice_column();

        let flag_start = meta.fixed_column();
        let flag_round = meta.fixed_column();
        let flag_final = meta.fixed_column();

        // ANCHOR: poseidon_start
        meta.create_gate("start", |meta| {
            let flag_start = meta.query_fixed(flag_start, Rotation::cur());
            let inp1 = meta.query_advice(inp1, Rotation::cur());
            let inp2 = meta.query_advice(inp2, Rotation::cur());
            let col1 = meta.query_advice(cols[0], Rotation::cur());
            let col2 = meta.query_advice(cols[1], Rotation::cur());
            let col3 = meta.query_advice(cols[2], Rotation::cur());
            vec![
                flag_start.clone() * (inp1 - col1), // col1 = inp1
                flag_start.clone() * (inp2 - col2), // col2 = inp2
                flag_start.clone() * col3,          // col3 = 0
            ]
        });
        // ANCHOR_END: poseidon_start

        // ANCHOR: poseidon_round1
        meta.create_gate("round", |meta| {
            let flag_round = meta.query_fixed(flag_round, Rotation::cur());

            let rndc = [
                meta.query_fixed(rndc[0], Rotation::cur()),
                meta.query_fixed(rndc[1], Rotation::cur()),
                meta.query_fixed(rndc[2], Rotation::cur()),
            ];

            let cols_cur = [
                meta.query_advice(cols[0], Rotation::cur()),
                meta.query_advice(cols[1], Rotation::cur()),
                meta.query_advice(cols[2], Rotation::cur()),
            ];

            let cols_nxt = [
                meta.query_advice(cols[0], Rotation::next()),
                meta.query_advice(cols[1], Rotation::next()),
                meta.query_advice(cols[2], Rotation::next()),
            ];

            let inp_cur = [
                meta.query_advice(inp1, Rotation::cur()),
                meta.query_advice(inp2, Rotation::cur()),
            ];

            let inp_nxt = [
                meta.query_advice(inp1, Rotation::next()),
                meta.query_advice(inp2, Rotation::next()),
            ];
            // ANCHOR_END: poseidon_round1

            // ANCHOR: poseidon_round_arc
            // add round constants
            let cols_arc = [
                cols_cur[0].clone() + rndc[0].clone(), //
                cols_cur[1].clone() + rndc[1].clone(), //
                cols_cur[2].clone() + rndc[2].clone(), //
            ];
            // ANCHOR_END: poseidon_round_arc

            // ANCHOR: poseidon_round_sbox
            // apply sbox: this is pretty inefficient, the degree of the gate is high
            assert_eq!(POWER, 5);

            fn sbox<F: Field>(x: Expression<F>) -> Expression<F> {
                x.clone() * x.clone() * x.clone() * x.clone() * x.clone()
            }

            let cols_sbox = [
                sbox(cols_arc[0].clone()),
                sbox(cols_arc[1].clone()),
                sbox(cols_arc[2].clone()),
            ];
            // ANCHOR_END: poseidon_round_sbox

            // ANCHOR: poseidon_round_matrix
            // apply matrix
            let cols_mat: [Expression<F>; WIDTH] = [
                Expression::Constant(F::ZERO)
                    + cols_sbox[0].clone() * matrix[0][0]
                    + cols_sbox[1].clone() * matrix[0][1]
                    + cols_sbox[2].clone() * matrix[0][2],
                Expression::Constant(F::ZERO)
                    + cols_sbox[0].clone() * matrix[1][0]
                    + cols_sbox[1].clone() * matrix[1][1]
                    + cols_sbox[2].clone() * matrix[1][2],
                Expression::Constant(F::ZERO)
                    + cols_sbox[0].clone() * matrix[2][0]
                    + cols_sbox[1].clone() * matrix[2][1]
                    + cols_sbox[2].clone() * matrix[2][2],
            ];
            // ANCHOR_END: poseidon_round_matrix

            // ANCHOR: poseidon_round_constraints
            // enforce that the next state is the round applied to the current state.
            vec![
                // round application
                flag_round.clone() * (cols_mat[0].clone() - cols_nxt[0].clone()), // inp1 = col1
                flag_round.clone() * (cols_mat[1].clone() - cols_nxt[1].clone()), // inp2 = col2
                flag_round.clone() * (cols_mat[2].clone() - cols_nxt[2].clone()), // 0 = col3
                // maintain input
                flag_round.clone() * (inp_cur[0].clone() - inp_nxt[0].clone()), // inp1 = inp1
                flag_round.clone() * (inp_cur[1].clone() - inp_nxt[1].clone()), // inp2 = inp2
            ]
        });
        // ANCHOR_END: poseidon_round_constraints

        Self {
            matrix,
            round_constants,
            _ph: PhantomData,
            flag_start,
            flag_round,
            flag_final,
            rndc,
            inp1,
            inp2,
            cols,
        }
    }

    // ANCHOR: poseidon_assign_row
    fn assign_row(
        &self,
        idx: usize,
        reg: &mut Region<'_, F>,
        flag_start: bool,
        flag_round: bool,
        flag_final: bool,
        rndc: [F; 3],
        cols: [F; 3],
        inp: [F; 2],
    ) -> Result<(), plonk::Error> {
        reg.assign_fixed(
            || "flag_start",
            self.flag_start,
            idx,
            || Value::known(if flag_start { F::ONE } else { F::ZERO }),
        )?;
        reg.assign_fixed(
            || "flag_round",
            self.flag_round,
            idx,
            || Value::known(if flag_round { F::ONE } else { F::ZERO }),
        )?;
        reg.assign_fixed(
            || "flag_final",
            self.flag_final,
            idx,
            || Value::known(if flag_final { F::ONE } else { F::ZERO }),
        )?;
        reg.assign_fixed(|| "rndc0", self.rndc[0], idx, || Value::known(rndc[0]))?;
        reg.assign_fixed(|| "rndc1", self.rndc[1], idx, || Value::known(rndc[1]))?;
        reg.assign_fixed(|| "rndc2", self.rndc[2], idx, || Value::known(rndc[2]))?;
        reg.assign_advice(|| "cols", self.cols[0], idx, || Value::known(cols[0]))?;
        reg.assign_advice(|| "cols", self.cols[1], idx, || Value::known(cols[1]))?;
        reg.assign_advice(|| "cols", self.cols[2], idx, || Value::known(cols[2]))?;
        reg.assign_advice(|| "inp1", self.inp1, idx, || Value::known(inp[0]))?;
        reg.assign_advice(|| "inp2", self.inp2, idx, || Value::known(inp[1]))?;
        Ok(())
    }
    // ANCHOR_END: poseidon_assign_row

    // ANCHOR: poseidon_populate
    fn populate(
        &self,
        layouter: &mut impl Layouter<F>,
        inputs: Vec<(F, F)>,
    ) -> Result<(), plonk::Error> {
        // ensure padded
        assert_eq!(inputs.len(), MAX_OPS_POSEIDON);

        // assign poseidon table
        layouter.assign_region(
            || "poseidon",
            |mut reg| {
                let mut st = [F::ZERO; WIDTH];
                let mut inp = [F::ZERO; 2];
                let mut nxt = 0;

                // zero row
                {
                    self.assign_row(
                        nxt,
                        &mut reg,
                        false,
                        false,
                        false,
                        [F::ZERO, F::ZERO, F::ZERO],
                        [F::ZERO, F::ZERO, F::ZERO],
                        [F::ZERO, F::ZERO],
                    )?;
                    nxt += 1;
                }

                for op in 0..MAX_OPS_POSEIDON {
                    // apply rounds
                    for r in 0..ROUNDS {
                        // load input
                        if r == 0 {
                            inp = [inputs[op].0, inputs[op].1];
                            st[0] = inp[0];
                            st[1] = inp[1];
                            st[2] = F::ZERO;
                        }

                        self.assign_row(
                            nxt,
                            &mut reg,
                            r == 0,
                            r > 0,
                            false,
                            self.round_constants[r],
                            st,
                            inp,
                        )?;

                        // apply poseidon round (out of circuit)
                        st = poseidon_round(&self.matrix, &self.round_constants[r], st);

                        // next row
                        nxt += 1;
                    }

                    // output
                    self.assign_row(
                        nxt,
                        &mut reg,
                        false,
                        false,
                        true,
                        [F::ZERO, F::ZERO, F::ZERO],
                        st,
                        inp,
                    )?;
                    nxt += 1;
                }
                Ok(())
            },
        )?;

        Ok(())
    }
    // ANCHOR_END: poseidon_populate
}

// ANCHOR: poseidon_chip
#[derive(Clone, Debug)]
pub struct PoseidonChip<F: Field> {
    inputs: RefCell<Vec<(F, F)>>,
    sel: Selector,
    tbl: PoseidonTable<F>,
    in1: Column<Advice>,
    in2: Column<Advice>,
    out: Column<Advice>,
    on: Column<Advice>,
}
// ANCHOR_END: poseidon_chip

// ANCHOR: poseidon_chip_configure
impl<F: Field> PoseidonChip<F> {
    fn configure(meta: &mut ConstraintSystem<F>) -> Self {
        let sel = meta.complex_selector();
        let in1 = meta.advice_column();
        let in2 = meta.advice_column();
        let out = meta.advice_column();
        let on = meta.advice_column();
        let tbl = PoseidonTable::new(meta);

        meta.enable_equality(in1);
        meta.enable_equality(in2);
        meta.enable_equality(out);
        meta.enable_equality(on);

        meta.create_gate("bit", |meta| {
            let on = meta.query_advice(on, Rotation::cur());
            let sel = meta.query_selector(sel);
            vec![
                sel * on.clone() * (on.clone() - Expression::Constant(F::ONE)), //
            ]
        });

        meta.lookup_any("poseidon_lookup", |cells| {
            let on = cells.query_advice(on, Rotation::cur());
            let sel = cells.query_selector(sel);
            let in1 = cells.query_advice(in1, Rotation::cur());
            let in2 = cells.query_advice(in2, Rotation::cur());
            let out = cells.query_advice(out, Rotation::cur());

            let do_lookup = on * sel;

            let table = tbl.table_expr(cells);

            // (1, in1, in2, out) in PoseidonTable
            vec![
                (do_lookup.clone() * Expression::Constant(F::ONE), table.flag),
                (do_lookup.clone() * in1.clone(), table.inp1),
                (do_lookup.clone() * in2.clone(), table.inp2),
                (do_lookup.clone() * out.clone(), table.out),
            ]
        });

        Self {
            sel,
            tbl,
            in1,
            in2,
            out,
            on,
            inputs: RefCell::new(Vec::new()),
        }
    }
    // ANCHOR_END: poseidon_chip_configure

    // ANCHOR: poseidon_chip_hash
    fn hash(
        &self,
        layouter: &mut impl Layouter<F>,
        on: AssignedCell<F, F>,
        in1: AssignedCell<F, F>,
        in2: AssignedCell<F, F>,
    ) -> Result<AssignedCell<F, F>, plonk::Error> {
        // store inputs
        in1.value().and_then(|in1| {
            in2.value()
                .map(|in2| self.inputs.borrow_mut().push((*in1, *in2)))
        });

        layouter.assign_region(
            || "poseidon",
            |mut reg| {
                self.sel.enable(&mut reg, 0)?;

                on.copy_advice(|| "on", &mut reg, self.on, 0)?;
                in1.copy_advice(|| "in1", &mut reg, self.in1, 0)?;
                in2.copy_advice(|| "in2", &mut reg, self.in2, 0)?;

                let hsh = in1
                    .value()
                    .and_then(|in1| in2.value().map(|in2| self.tbl.hash(*in1, *in2)));

                // if on = 0, hsh = 0
                let hsh = on.value().and_then(|on| hsh.map(|hsh| hsh * on));

                let out = reg.assign_advice(|| "out", self.out, 0, || hsh)?;
                Ok(out)
            },
        )
    }
    // ANCHOR_END: poseidon_chip_hash

    // ANCHOR: poseidon_chip_finalize
    fn finalize(self, layouter: &mut impl Layouter<F>) -> Result<(), plonk::Error> {
        let mut inputs = self.inputs.borrow().clone();
        while inputs.len() < MAX_OPS_POSEIDON {
            inputs.push((F::ZERO, F::ZERO));
        }
        self.tbl.populate(layouter, inputs)
    }
    // ANCHOR_END: poseidon_chip_finalize
}

// ANCHOR: test_circuit
impl<F: Field> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit { _ph: PhantomData }
    }

    #[allow(unused_variables)]
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let poseidon = PoseidonChip::configure(meta);
        let free = meta.advice_column();
        meta.enable_equality(free);
        TestConfig { poseidon, free }
    }

    #[allow(unused_variables)]
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), plonk::Error> {
        let hashes = vec![(F::ZERO, F::ZERO); MAX_OPS_POSEIDON];

        let in1 = layouter.assign_region(
            || "free1",
            |mut region| {
                region.assign_advice(
                    || "free", //
                    config.free,
                    0,
                    || Value::known(F::ONE),
                )
            },
        )?;

        let in2 = layouter.assign_region(
            || "free2",
            |mut region| {
                region.assign_advice(
                    || "free", //
                    config.free,
                    0,
                    || Value::known(F::ONE),
                )
            },
        )?;

        let on = layouter.assign_region(
            || "free3",
            |mut region| {
                region.assign_advice(
                    || "free", //
                    config.free,
                    0,
                    || Value::known(F::ONE),
                )
            },
        )?;

        // populate poseidon
        let out = config.poseidon.hash(&mut layouter, on, in1, in2)?;
        println!("hash done: {:?}", out);
        config.poseidon.finalize(&mut layouter)?;
        Ok(())
    }
}
// ANCHOR_END: test_circuit

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;
    let circuit = TestCircuit::<Fr> { _ph: PhantomData };
    let prover = MockProver::run(12, &circuit, vec![]).unwrap();
    prover.verify().unwrap();
}
