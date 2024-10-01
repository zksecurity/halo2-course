use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{layouter, AssignedCell, Layouter, Region, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        self, Advice, Circuit, Column, ConstraintSystem, Error, Expression, Selector, TableColumn,
    },
    poly::Rotation,
};

use ff::{BitViewSized, Field, PrimeFieldBits};

#[derive(Clone, Debug)]
struct RangeTable<F: PrimeFieldBits, const BITS: usize> {
    range: TableColumn,
    _ph: PhantomData<F>,
}

// Table with (0, 1, 2, 3, 2^BITS-1) as rows
impl<F: PrimeFieldBits, const BITS: usize> RangeTable<F, BITS> {
    fn configure(meta: &mut ConstraintSystem<F>) -> Self {
        let range = meta.lookup_table_column();
        Self {
            _ph: PhantomData,
            range,
        }
    }

    fn load(&self, layouter: &mut impl Layouter<F>) -> Result<(), Error> {
        layouter.assign_table(
            || "load range-check table",
            |mut table| {
                let mut offset = 0;
                for value in 0..(1 << BITS) {
                    table.assign_cell(
                        || "val_in_range",
                        self.range,
                        offset,
                        || Value::known(F::from(value as u64)),
                    )?;
                    offset += 1;
                }
                Ok(())
            },
        )
    }
}

#[derive(Clone, Debug)]
struct RangeConfig<F: PrimeFieldBits, const BITS: usize, const LIMBS: usize> {
    value: Column<Advice>,
    limbs: [Column<Advice>; LIMBS],
    table: RangeTable<F, BITS>,
    q_enable: Selector,
    _ph: PhantomData<F>,
}

// v in [0, B] : B < 2^(BITS * LIMBS)
//
// v     < 2^(BITS * LIMBS)
// B - v < 2^(BITS * LIMBS)
//
// Chip can check: 0 <= v < 2^(BITS * LIMBS)
impl<F: PrimeFieldBits, const BITS: usize, const LIMBS: usize> RangeConfig<F, BITS, LIMBS> {
    fn configure(
        meta: &mut ConstraintSystem<F>,
        value: Column<Advice>,
        table: RangeTable<F, BITS>,
        limbs: [Column<Advice>; LIMBS],
    ) -> RangeConfig<F, BITS, LIMBS> {
        let q_enable = meta.complex_selector();
        meta.enable_equality(value);

        // check decomposition
        meta.create_gate("combine", |meta| {
            let value = meta.query_advice(value, Rotation::cur());
            let q_enable = meta.query_selector(q_enable);

            // combine = 2^0 * limbs[0] + 2^BITS * limbs[1] + 2^(2*BITS) * limbs[2] + ...
            let mut power = F::ONE;
            let mut combine = Expression::Constant(F::ZERO);
            for limb in limbs.iter().cloned() {
                let limb = meta.query_advice(limb, Rotation::cur());
                combine = combine + Expression::Constant(power) * limb;
                power *= &F::from_u128(1 << BITS as u128);
            }
            vec![(combine - value) * q_enable]
        });

        // lookup every limb
        for limb in limbs.iter().cloned() {
            // limbi in table.range
            meta.lookup("lookup_limb", |meta| {
                let limb = meta.query_advice(limb, Rotation::cur());
                let q_enable = meta.query_selector(q_enable);
                vec![(q_enable * limb, table.range)]
            });
        }

        RangeConfig {
            value,
            table,
            q_enable,
            limbs,
            _ph: PhantomData,
        }
    }

    fn check(
        &self,
        layouter: &mut impl Layouter<F>,
        value: &AssignedCell<F, F>,
    ) -> Result<(), Error> {
        assert!(BITS * LIMBS <= F::CAPACITY as usize);

        // decompose value into limbs
        let limbs: Value<[F; LIMBS]> = value.value().map(|v| {
            let le_bits = v.clone().to_le_bits();
            let le_bits: Vec<_> = le_bits.iter().take(LIMBS * BITS).collect();
            let mut limbs = Vec::with_capacity(LIMBS);
            for limb in le_bits.chunks_exact(BITS) {
                let mut v = 0;
                for (i, bit) in limb.into_iter().enumerate() {
                    if **bit {
                        v += 1 << i;
                    }
                }
                limbs.push(F::from_u128(v));
            }

            assert_eq!(limbs.len(), LIMBS);
            limbs.try_into().unwrap()
        });

        // assign all the decomposed limbs
        layouter.assign_region(
            || "check_range",
            |mut region| {
                self.q_enable.enable(&mut region, 0)?;
                value.copy_advice(|| "", &mut region, self.value, 0)?; //
                for (i, limb) in self.limbs.iter().cloned().enumerate() {
                    region.assign_advice(|| "limb", limb, 0, || limbs.map(|l| l[i]))?;
                }
                Ok(())
            },
        )
    }
}

// (addr, rw_counter, val_old, val_new, is_write)
// Want: this to be sorted according to (addr, rw_counter)
#[derive(Clone, Debug)]
struct RwTable<F: PrimeFieldBits, const ROWS: usize> {
    q_enable: Selector,         // is the RwTable defined for this row?
    addr: Column<Advice>,       // address of the cell
    rw_counter: Column<Advice>, // counter of the row
    val_old: Column<Advice>,    // prev. value of the cell
    val_new: Column<Advice>,    // next value of the cell
    is_write: Column<Advice>,   // is this a write?
    _ph: PhantomData<F>,
}

#[derive(Clone, Debug)]
struct RwRow<F: PrimeFieldBits> {
    addr: u32,
    val_new: F,
    val_old: F,
    rw_counter: u32,
    is_write: bool,
}

impl<F: PrimeFieldBits> RwRow<F> {
    fn key(&self) -> u64 {
        (self.addr as u64) << 32 | self.rw_counter as u64
    }
}

impl<F: PrimeFieldBits, const ROWS: usize> RwTable<F, ROWS> {
    fn configure(meta: &mut ConstraintSystem<F>) -> Self {
        let q_enable = meta.selector();

        let addr = meta.advice_column();
        let val_old = meta.advice_column();
        let val_new = meta.advice_column();
        let rw_counter = meta.advice_column();
        let is_write = meta.advice_column();

        Self {
            addr,
            val_old,
            val_new,
            rw_counter,
            is_write,
            q_enable,
            _ph: PhantomData,
        }
    }

    fn assign_with_region(
        &self,
        rows: Value<Vec<RwRow<F>>>,
        region: &mut Region<'_, F>,
    ) -> Result<(), Error> {
        for i in 0..ROWS {
            // turn on the row
            if i != ROWS - 1 {
                self.q_enable.enable(region, i)?;
            }

            // assign combined key
            region.assign_advice(
                || format!("addr[{}]", i),
                self.addr,
                i,
                || {
                    rows.as_ref().map(|m| {
                        let v: F = (m[i].addr as u64).into();
                        v
                    })
                },
            )?;
            region.assign_advice(
                || format!("rw_counter[{}]", i),
                self.rw_counter,
                i,
                || {
                    rows.as_ref().map(|m| {
                        let v: F = (m[i].rw_counter as u64).into();
                        v
                    })
                },
            )?;
            region.assign_advice(
                || format!("value_old[{}]", i),
                self.val_old,
                i,
                || rows.as_ref().map(|m| m[i].val_old),
            )?;
            region.assign_advice(
                || format!("value_new[{}]", i),
                self.val_new,
                i,
                || rows.as_ref().map(|m| m[i].val_new),
            )?;
            region.assign_advice(
                || format!("is_write[{}]", i),
                self.is_write,
                i,
                || {
                    rows.as_ref()
                        .map(|m| if m[i].is_write { F::ONE } else { F::ZERO })
                },
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct StateConfig<F: PrimeFieldBits, const ROWS: usize> {
    rw_table: RwTable<F, ROWS>,
    range64: RangeConfig<F, 8, 8>,
    delta: Column<Advice>,
}

impl<F: PrimeFieldBits, const ROWS: usize> StateConfig<F, ROWS> {
    fn configure(
        meta: &mut ConstraintSystem<F>,
        rw_table: RwTable<F, ROWS>,
        range64: RangeConfig<F, 8, 8>,
    ) -> Self {
        let delta = meta.advice_column();

        meta.enable_equality(delta);

        meta.create_gate("delta_gate", |meta| {
            let delta = meta.query_advice(delta, Rotation::cur());
            let q_enable = meta.query_selector(rw_table.q_enable);

            let addr_cur = meta.query_advice(rw_table.addr, Rotation::cur());
            let addr_nxt = meta.query_advice(rw_table.addr, Rotation::next());

            let rw_cur = meta.query_advice(rw_table.rw_counter, Rotation::cur());
            let rw_nxt = meta.query_advice(rw_table.rw_counter, Rotation::next());

            let key_cur = Expression::Constant(F::from_u128(1 << 32)) * addr_cur + rw_cur;
            let key_nxt = Expression::Constant(F::from_u128(1 << 32)) * addr_nxt + rw_nxt;

            vec![q_enable * (delta - (key_nxt - key_cur))]
        });

        Self {
            rw_table,
            delta,
            range64,
        }
    }

    fn assign(
        &self,
        rows: Value<Vec<RwRow<F>>>,
        layouter: &mut impl Layouter<F>,
    ) -> Result<(), Error> {
        let range_64 = layouter.assign_region(
            || "state",
            |mut region| {
                // assigns the RwTable
                self.rw_table
                    .assign_with_region(rows.clone(), &mut region)?;

                //
                let deltas: Value<Vec<u64>> = rows.as_ref().map(|rows| {
                    rows.windows(2)
                        .map(|win| {
                            let cur = &win[0];
                            let nxt = &win[1];
                            nxt.key().wrapping_sub(cur.key())
                        })
                        .collect()
                });

                // assign deltas
                let mut range_64 = Vec::with_capacity(ROWS - 1);
                for i in 0..ROWS - 1 {
                    range_64.push(region.assign_advice(
                        || format!("delta[{}]", i),
                        self.delta,
                        i,
                        || {
                            deltas.as_ref().map(|m| {
                                let v: F = m[i].into();
                                v
                            })
                        },
                    )?);
                }

                Ok(range_64)
            },
        )?;

        // add all the range checks
        for cell in range_64.iter() {
            self.range64.check(layouter, cell)?;
        }

        Ok(())
    }
}

struct TestCircuit<F: PrimeFieldBits> {
    _ph: PhantomData<F>,
    rw_table: Value<Vec<RwRow<F>>>,
}

#[derive(Clone, Debug)]
struct TestConfig<F: PrimeFieldBits + Clone> {
    value: Column<Advice>,
    tabl_range: RangeTable<F, 8>,
    chip_range: RangeConfig<F, 8, 8>,
    rw_table: RwTable<F, 4>,
    state: StateConfig<F, 4>,
    _ph: PhantomData<F>,
}

impl<F: PrimeFieldBits> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            rw_table: Value::unknown(),
        }
    }

    #[allow(unused_variables)]
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let value = meta.advice_column();
        let limbs = [(); 8].map(|_| meta.advice_column());
        let tabl_range = RangeTable::<F, 8>::configure(meta);
        let chip_range = RangeConfig::configure(meta, value, tabl_range.clone(), limbs);

        let rw_table = RwTable::<F, 4>::configure(meta);
        let state = StateConfig::<F, 4>::configure(meta, rw_table.clone(), chip_range.clone());

        TestConfig {
            _ph: PhantomData,
            value,
            tabl_range,
            chip_range,
            rw_table,
            state,
        }
    }

    #[allow(unused_variables)]
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), plonk::Error> {
        config.tabl_range.load(&mut layouter)?;

        let free = layouter.assign_region(
            || "test",
            |mut region| {
                region.assign_advice(
                    || "test",
                    config.value,
                    0,
                    || {
                        let v: F = 10_000u64.into();
                        Value::known(v)
                    },
                )
            },
        )?;

        config.chip_range.check(&mut layouter, &free)?;

        config.state.assign(self.rw_table.clone(), &mut layouter)?;

        /*
        layouter.assign_region(
            || "rw_table",
            |mut region| {
                config
                    .rw_table
                    .assign_with_region(self.rw_table.clone(), &mut region)
            },
        )?;
        */

        Ok(())
    }
}

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    let rw_rows = vec![
        RwRow {
            addr: 0,
            val_old: Fr::from(0u64),
            val_new: Fr::from(1u64),
            rw_counter: 0,
            is_write: true,
        },
        RwRow {
            addr: 0,
            val_old: Fr::from(1u64),
            val_new: Fr::from(1u64),
            rw_counter: 2,
            is_write: false,
        },
        RwRow {
            addr: 1,
            val_old: Fr::from(0u64),
            val_new: Fr::from(2u64),
            rw_counter: 1,
            is_write: true,
        },
        RwRow {
            addr: 2,
            val_old: Fr::from(0u64),
            val_new: Fr::from(3u64),
            rw_counter: 3,
            is_write: true,
        },
    ];

    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        rw_table: Value::known(rw_rows),
    };
    let prover = MockProver::run(16, &circuit, vec![]).unwrap();
    prover.verify().unwrap();
}
