use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{AssignedCell, Layouter, Region, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{
        self, Advice, Circuit, Column, ConstraintSystem, Expression, Fixed, Selector, VirtualCells,
    },
    poly::Rotation,
};

use ff::{Field, PrimeField};

// Gate:
//
// Prover can choose an array
//
// Get(index) = array[index]
//
// Get(0) == 0xcafecafe

const MAX_MEMORY: usize = 5;

// (0, ?)
// (1, ?)
// (2, ?)
// ...
#[derive(Clone, Debug)]
struct RomTable<F: Field> {
    idx: Column<Fixed>,
    arr: Column<Advice>,
    flag: Column<Fixed>,
    _ph: PhantomData<F>,
}

impl<F: PrimeField> RomTable<F> {
    fn configure(meta: &mut ConstraintSystem<F>) -> Self {
        let idx = meta.fixed_column();
        let arr = meta.advice_column();
        let flag = meta.fixed_column();

        Self {
            idx,
            arr,
            flag,
            _ph: PhantomData,
        }
    }

    fn assign_row(
        &self,
        region: &mut Region<'_, F>,
        i: usize,
        on: bool,
        idx: Value<F>,
        arr: Value<F>,
    ) -> Result<(), plonk::Error> {
        region.assign_fixed(|| "idx", self.idx, i, || idx)?;
        region.assign_advice(|| "arr", self.arr, i, || arr)?;
        region.assign_fixed(
            || "on",
            self.flag,
            i,
            || Value::known(if on { F::ONE } else { F::ZERO }),
        )?;
        Ok(())
    }

    fn populate(
        &self,
        layouter: &mut impl Layouter<F>,
        memory: Value<&Vec<F>>,
    ) -> Result<(), plonk::Error> {
        memory.assert_if_known(|m| m.len() == MAX_MEMORY);

        layouter.assign_region(
            || "memory",
            |mut region| {
                for i in 0..MAX_MEMORY {
                    println!("Assigning row {}", i);
                    println!("Memory: {:?}", memory.as_ref().map(|m| m[i]));
                    println!("index: {:?}", F::from_u128(i as u128));
                    self.assign_row(
                        &mut region,
                        i,
                        true,
                        Value::known(F::from_u128(i as u128)),
                        memory.as_ref().map(|m| m[i]),
                    )?;
                }

                self.assign_row(
                    &mut region,
                    MAX_MEMORY,
                    false,
                    Value::known(F::ZERO),
                    Value::known(F::ZERO),
                )?;

                Ok(())
            },
        )?;
        Ok(())
    }

    fn lookup_expr(
        &self,
        cells: &mut VirtualCells<F>,
    ) -> (Expression<F>, Expression<F>, Expression<F>) {
        let flag = cells.query_fixed(self.flag, Rotation::cur());
        let idx = cells.query_fixed(self.idx, Rotation::cur());
        let arr = cells.query_advice(self.arr, Rotation::cur());
        (flag, idx, arr)
    }
}

#[derive(Clone, Debug)]
struct RomChip<F: Field> {
    rom_enable: Selector,
    rom: RomTable<F>,
    output: Column<Advice>,
    input: Column<Advice>,
    _ph: PhantomData<F>,
}

#[derive(Clone, Debug)]
struct Index<F: Field> {
    assigned: AssignedCell<F, F>,
    value: Value<usize>,
}

impl<F: PrimeField> RomChip<F> {
    fn configure(
        meta: &mut ConstraintSystem<F>,
        rom: RomTable<F>,
        output: Column<Advice>, // the output: the value at the index
        input: Column<Advice>,  // the input: the index
    ) -> Self {
        let rom_enable = meta.complex_selector();

        meta.lookup_any("ROM lookup", |meta| {
            let enabled = meta.query_selector(rom_enable);
            let input = meta.query_advice(input, Rotation::cur());
            let output = meta.query_advice(output, Rotation::cur());
            let (flag, idx, arr) = rom.lookup_expr(meta);

            // (1, input, output) in (flag, idx, arr)
            vec![
                (enabled.clone(), flag),
                (enabled.clone() * input, idx),
                (enabled.clone() * output, arr),
            ]
        });

        Self {
            rom_enable,
            rom,
            output,
            input,
            _ph: PhantomData,
        }
    }

    // y = arr[i]
    //
    // pick y
    // (y, i) in (arr, idx)
    // return y
    fn get(
        &self,
        layouter: &mut impl Layouter<F>,
        rom: &Value<Vec<F>>,
        input: Index<F>,
    ) -> Result<AssignedCell<F, F>, plonk::Error> {
        layouter.assign_region(
            || "get",
            |mut region| {
                self.rom_enable.enable(&mut region, 0)?;

                println!("");
                println!("input: {:?}", input);

                input
                    .assigned
                    .copy_advice(|| "input", &mut region, self.input, 0)?; //

                let output = input.value.and_then(|i| rom.as_ref().map(|m| m[i]));

                println!("output: {:?}", output);

                let output = region.assign_advice(|| "output", self.output, 0, || output)?;

                println!("rom: {:?}", rom);
                println!("output: {:?}", output);
                println!("");

                Ok(output)
            },
        )
    }
}

struct TestCircuit<F: Field> {
    _ph: PhantomData<F>,
    rom: Value<Vec<F>>,
}

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    rom: RomTable<F>,
    rom_chip: RomChip<F>,
    adv1: Column<Advice>,
    adv2: Column<Advice>,
}

impl<F: PrimeField> TestConfig<F> {
    fn free_index(
        &self,
        layouter: &mut impl Layouter<F>,
        idx: Value<usize>,
    ) -> Result<Index<F>, plonk::Error> {
        let assigned = layouter.assign_region(
            || "index",
            |mut region| {
                region.assign_advice(
                    || "index",
                    self.adv1,
                    0,
                    || idx.map(|i| F::from_u128(i as u128)),
                )
            },
        )?;

        Ok(Index {
            assigned,
            value: idx,
        })
    }
}

impl<F: PrimeField> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            rom: self.rom.clone(),
        }
    }

    #[allow(unused_variables)]
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let adv1 = meta.advice_column();
        let adv2 = meta.advice_column();
        let rom = RomTable::configure(meta);
        let rom_chip = RomChip::configure(meta, rom.clone(), adv1, adv2);

        meta.enable_equality(adv1);
        meta.enable_equality(adv2);

        TestConfig {
            _ph: PhantomData {},
            rom,
            rom_chip,
            adv1,
            adv2,
        }
    }

    #[allow(unused_variables)]
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), plonk::Error> {
        // assign the ROM
        config.rom.populate(&mut layouter, self.rom.as_ref())?;

        let idx1 = config.free_index(&mut layouter, Value::known(1))?;
        let idx2 = config.free_index(&mut layouter, Value::known(1))?;

        let arr1 = config.rom_chip.get(&mut layouter, &self.rom, idx1)?;
        /*
        let arr2 = config.rom_chip.get(&mut layouter, &self.rom, idx2)?;

        println!("arr1: {:?}", arr1);
        println!("arr2: {:?}", arr2);
        */

        Ok(())
    }
}

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    use std::iter;

    let rom = "Hello"; // World!";
    let rom: Vec<Fr> = rom.chars().map(|c| Fr::from_u128(c as u128)).collect();
    let rom = rom
        .into_iter()
        .chain(iter::repeat(Fr::ZERO))
        .take(MAX_MEMORY)
        .collect::<Vec<_>>();

    let rom = Value::known(rom);

    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        rom,
    };
    let prover = MockProver::run(12, &circuit, vec![]).unwrap();
    prover.verify().unwrap();
}
