use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        Advice,
        Circuit,
        Column, //
        ConstraintSystem,
        Error,
        Fixed,
        Selector,
        TableColumn,
    },
    poly::Rotation,
};

use ff::{Field, PrimeField};

// ANCHOR: rwtable
struct RwTable {
    addr: Column<Advice>,    // address
    value: Column<Advice>,   // value
    counter: Column<Advice>, // counter
}
// ANCHOR_END: rwtable

// ANCHOR: regex
const ST_A: usize = 1;
const ST_B: usize = 2;
const ST_C: usize = 3;

// start and done states
const ST_START: usize = ST_A;
const ST_DONE: usize = 4;

// end of file marker:
// "dummy padding character"
const EOF: usize = 0xFFFF;

// conversion of the regular expression: a+b+c
const REGEX: [(usize, usize, Option<char>); 6] = [
    (ST_A, ST_A, Some('a')),    // you can stay in ST_A by reading 'a'
    (ST_A, ST_B, Some('a')),    // or move to ST_B by reading 'a'
    (ST_B, ST_B, Some('b')),    // you can stay in ST_B by reading 'b'
    (ST_B, ST_C, Some('b')),    // or move to ST_C by reading 'b'
    (ST_C, ST_DONE, Some('c')), // you can move to ST_DONE by reading 'c'
    (ST_DONE, ST_DONE, None),   // you can stay in ST_DONE by reading EOF
];
// ANCHOR_END: regex

const MAX_STR_LEN: usize = 20;

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    str: Value<String>,
    sts: Value<Vec<usize>>,
}

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    q_match: Selector,
    q_regex: Selector,  // enable the regex gate
    st: Column<Advice>, // current state of automaton
    ch: Column<Advice>, // current character
    tbl_st_cur: TableColumn,
    tbl_st_nxt: TableColumn,
    tbl_ch: TableColumn,
    fix_st: Column<Fixed>,
}

impl<F: PrimeField> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            str: Value::unknown(), // the string
            sts: Value::unknown(), // state of the automaton
        }
    }

    // ANCHOR: columns
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let q_regex = meta.complex_selector();
        let q_match = meta.complex_selector();

        let st = meta.advice_column();
        let ch = meta.advice_column();

        let fix_st = meta.fixed_column();

        let tbl_st_cur = meta.lookup_table_column();
        let tbl_st_nxt = meta.lookup_table_column();
        let tbl_ch = meta.lookup_table_column();

        // ANCHOR_END: columns
        // ANCHOR: lookup
        meta.lookup("step", |meta| {
            let st_cur = meta.query_advice(st, Rotation::cur());
            let st_nxt = meta.query_advice(st, Rotation::next());
            let ch = meta.query_advice(ch, Rotation::cur());
            let en = meta.query_selector(q_regex);
            vec![
                (en.clone() * st_cur, tbl_st_cur),
                (en.clone() * st_nxt, tbl_st_nxt),
                (en.clone() * ch, tbl_ch),
            ]
        });
        // ANCHOR_END: lookup

        meta.create_gate("fix state", |meta| {
            let st = meta.query_advice(st, Rotation::cur());
            let fix_st = meta.query_fixed(fix_st, Rotation::cur());
            let en = meta.query_selector(q_match);
            vec![en * (st - fix_st)]
        });

        TestConfig {
            _ph: PhantomData,
            q_regex,
            st,
            ch,
            tbl_st_cur,
            tbl_st_nxt,
            tbl_ch,
            fix_st,
            q_match,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // assign the transition table
        layouter.assign_table(
            || "table",
            |mut table| {
                // a table of field elements and their "pop counts"
                // (a not very field friendly operation)
                let mut transitions: Vec<(F, F, F)> = vec![
                    // (0, 0, 0) is in the table to account for q_regex = 0
                    (F::ZERO, F::ZERO, F::ZERO),
                ];
                for tx in REGEX.iter() {
                    let (st_cur, st_nxt, ch) = tx;
                    transitions.push((
                        F::from(*st_cur as u64),
                        F::from(*st_nxt as u64),
                        ch.map(|c| F::from(c as u64)).unwrap_or(F::from(EOF as u64)),
                    ));
                }

                // assign the table
                for (offset, (st_cur, st_nxt, char)) in transitions //
                    .into_iter()
                    .enumerate()
                {
                    table.assign_cell(
                        || format!("key"),
                        config.tbl_st_cur,
                        offset,
                        || Value::known(st_cur),
                    )?;
                    table.assign_cell(
                        || format!("value"),
                        config.tbl_st_nxt,
                        offset,
                        || Value::known(st_nxt),
                    )?;
                    table.assign_cell(
                        || format!("char"),
                        config.tbl_ch,
                        offset,
                        || Value::known(char),
                    )?;
                }
                Ok(())
            },
        )?;

        // create a region which can check the regex expression
        // note: you could have multiple regions to check
        // the same regex at basically no additional cost
        layouter.assign_region(
            || "regex",
            |mut region| {
                // at offset 0, the state is ST_START
                region.assign_fixed(
                    || "initial state",
                    config.fix_st,
                    0,
                    || Value::known(F::from(ST_START as u64)),
                )?;
                config.q_match.enable(&mut region, 0)?;

                // assign each step
                for i in 0..MAX_STR_LEN {
                    // enable the regex automaton
                    config.q_regex.enable(&mut region, i)?;

                    // state
                    region.assign_advice(
                        || "st",
                        config.st,
                        i,
                        || {
                            self.sts.as_ref().map(|s| {
                                F::from(
                                    s.get(i) //
                                        .cloned()
                                        .unwrap_or(ST_DONE)
                                        as u64,
                                )
                            })
                        },
                    )?;

                    // character
                    region.assign_advice(
                        || "ch",
                        config.ch,
                        i,
                        || {
                            self.str.as_ref().map(|s| {
                                s.chars()
                                    .nth(i)
                                    .map(|c| F::from(c as u64))
                                    .unwrap_or(F::from(EOF as u64))
                            })
                        },
                    )?;
                }

                // at offset MAX_STR_LEN, the state is ST_START
                region.assign_advice(
                    || "st",
                    config.st,
                    MAX_STR_LEN,
                    || Value::known(F::from(ST_DONE as u64)),
                )?;
                region.assign_fixed(
                    || "final state",
                    config.fix_st,
                    MAX_STR_LEN,
                    || Value::known(F::from(ST_DONE as u64)),
                )?;
                config.q_match.enable(&mut region, MAX_STR_LEN)?;

                Ok(())
            },
        )?;

        Ok(())
    }
}

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    // run the MockProver
    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        // the string to match
        str: Value::known("aaabbbc".to_string()),
        // manually create a trace of the state transitions
        sts: Value::known(vec![
            ST_A,    // ST_A -a-> ST_A (START)
            ST_A,    // ST_A -a-> ST_A
            ST_A,    // ST_A -a-> ST_A
            ST_B,    // ST_A -a-> ST_B
            ST_B,    // ST_B -b-> ST_B
            ST_B,    // ST_B -b-> ST_B
            ST_C,    // ST_B -b-> ST_C
            ST_DONE, // ST_C -c-> ST_DONE
        ]),
    };
    let prover = MockProver::run(8, &circuit, vec![]).unwrap();
    prover.verify().unwrap();
}
