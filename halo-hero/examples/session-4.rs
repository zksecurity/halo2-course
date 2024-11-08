use std::{iter, marker::PhantomData};

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
        TableColumn,
    },
    poly::Rotation,
};

use ff::{Field, PrimeField};

struct TestCircuit<F: Field> {
    key: Value<[u8; 16]>,
    pt: Value<[u8; 16]>,
    _ph: PhantomData<F>,
}

#[derive(Clone, Debug)]
struct Byte<F: PrimeField> {
    value: Value<u8>,
    cell: AssignedCell<F, F>,
}

#[derive(Clone, Debug)]
struct LookupChip<F: Field> {
    enable: Selector,
    typ: TableColumn,
    in1: TableColumn,
    in2: TableColumn,
    out: TableColumn,
    entryt: Column<Fixed>,
    input1: Column<Advice>,
    input2: Column<Advice>,
    output: Column<Advice>,
    _ph: PhantomData<F>,
}

impl<F: PrimeField> TestConfig<F> {
    fn free(&self, layouter: &mut impl Layouter<F>, value: Value<u8>) -> Result<Byte<F>, Error> {
        layouter.assign_region(
            || "free byte",
            |mut region| {
                let cell = region.assign_advice(
                    || "byte",
                    self.input1,
                    0,
                    || value.map(|v| F::from_u128(v as u128)),
                )?;
                Ok(Byte { value, cell })
            },
        )
    }
}

impl<F: PrimeField> LookupChip<F> {
    fn configure(
        meta: &mut ConstraintSystem<F>,
        fixed: Column<Fixed>,
        input1: Column<Advice>,
        input2: Column<Advice>,
        output: Column<Advice>,
    ) -> Self {
        let enable = meta.complex_selector();
        let typ = meta.lookup_table_column();
        let in1 = meta.lookup_table_column();
        let in2 = meta.lookup_table_column();
        let out = meta.lookup_table_column();

        meta.lookup("op", |meta| {
            let enable = meta.query_selector(enable);
            let input1 = meta.query_advice(input1, Rotation::cur());
            let input2 = meta.query_advice(input2, Rotation::cur());
            let output = meta.query_advice(output, Rotation::cur());
            let entryt = meta.query_fixed(fixed, Rotation::cur());
            vec![
                (enable.clone() * entryt, typ),
                (enable.clone() * input1, in1),
                (enable.clone() * input2, in2),
                (enable.clone() * output, out),
            ]
        });

        Self {
            enable,
            typ,
            in1,
            in2,
            out,
            entryt: fixed,
            input1,
            input2,
            output,
            _ph: PhantomData,
        }
    }

    // Populate the lookup table with the required operation for AES
    fn initialize(&self, layouter: &mut impl Layouter<F>) -> Result<(), Error> {
        let mut entries = Vec::new();

        // XOR
        for i in 0..=0xff {
            for j in 0..=0xff {
                entries.push((
                    TYP_XOR,
                    F::from_u128(i as u128),
                    F::from_u128(j as u128),
                    F::from_u128(i ^ j),
                ));
            }
        }

        // MUL2
        for inp in 0..=0xff {
            entries.push((
                TYP_MUL2,
                F::from_u128(inp as u128),
                F::from_u128(0),
                F::from_u128(op_mul2(inp) as u128),
            ));
        }

        // MUL3
        for inp in 0..=0xff {
            entries.push((
                TYP_MUL3,
                F::from_u128(inp as u128),
                F::from_u128(0),
                F::from_u128(op_mul3(inp) as u128),
            ));
        }

        // SBOX
        for inp in 0..=0xff {
            entries.push((
                TYP_SBOX,
                F::from_u128(inp as u128),
                F::from_u128(0),
                F::from_u128(SBOX[inp as usize] as u128),
            ));
        }

        layouter.assign_table(
            || "aes lookups",
            |mut tbl| {
                // add the zero row
                tbl.assign_cell(|| "typ", self.typ, 0, || Value::known(F::ZERO))?;
                tbl.assign_cell(|| "in1", self.in1, 0, || Value::known(F::ZERO))?;
                tbl.assign_cell(|| "in2", self.in2, 0, || Value::known(F::ZERO))?;
                tbl.assign_cell(|| "out", self.out, 0, || Value::known(F::ZERO))?;

                // add the rest of the entries
                let mut nxt = 1;
                for (typ, inp1, inp2, outp) in entries.iter().cloned() {
                    tbl.assign_cell(
                        || "typ",
                        self.typ,
                        nxt,
                        || Value::known(F::from_u128(typ as u128)),
                    )?;
                    tbl.assign_cell(|| "in1", self.in1, nxt, || Value::known(inp1))?;
                    tbl.assign_cell(|| "in2", self.in2, nxt, || Value::known(inp2))?;
                    tbl.assign_cell(|| "out", self.out, nxt, || Value::known(outp))?;
                    nxt += 1;
                }
                Ok(())
            },
        )
    }

    fn xor(
        &self,
        layouter: &mut impl Layouter<F>,
        inp1: Byte<F>,
        inp2: Byte<F>,
    ) -> Result<Byte<F>, Error> {
        layouter.assign_region(
            || "xor",
            |mut reg| {
                self.enable.enable(&mut reg, 0)?;
                reg.assign_fixed(
                    || "typ",
                    self.entryt,
                    0,
                    || Value::known(F::from_u128(TYP_XOR as u128)),
                )?;
                inp1.cell.copy_advice(|| "inp1", &mut reg, self.input1, 0)?;
                inp2.cell.copy_advice(|| "inp2", &mut reg, self.input2, 0)?;

                // compute value = inp1 ^ inp2
                let value = inp1
                    .value
                    .and_then(|a| inp2.value.and_then(|b| Value::known(a ^ b)));

                // assign value to output
                let assigned = reg.assign_advice(
                    || "out",
                    self.output,
                    0,
                    || value.map(|v| F::from_u128(v as u128)),
                )?;

                Ok(Byte {
                    value,
                    cell: assigned,
                })
            },
        )
    }

    fn sbox(&self, layouter: &mut impl Layouter<F>, inp: Byte<F>) -> Result<Byte<F>, Error> {
        //
        layouter.assign_region(
            || "xor",
            |mut reg| {
                self.enable.enable(&mut reg, 0)?;

                todo!("Some stuff missing here");

                // a little hint to get you started ;)
                reg.assign_advice(|| "inp2", self.input2, 0, || Value::known(F::ZERO))?;

                // compute value = sbox[inp1]
                let value: Value<u8> = todo!("?");

                // assign value to output
                let assigned = reg.assign_advice(
                    || "out",
                    self.output,
                    0,
                    || value.map(|v| F::from_u128(v as u128)),
                )?;

                Ok(Byte {
                    value,
                    cell: assigned,
                })
            },
        )
    }

    fn mul2(&self, layouter: &mut impl Layouter<F>, inp: Byte<F>) -> Result<Byte<F>, Error> {
        todo!("fill me in")
    }

    fn mul3(&self, layouter: &mut impl Layouter<F>, inp: Byte<F>) -> Result<Byte<F>, Error> {
        todo!("fill me in")
    }

    fn mix_row(
        &self,
        layouter: &mut impl Layouter<F>,
        m2: Byte<F>,
        m3: Byte<F>,
        add1: Byte<F>,
        add2: Byte<F>,
    ) -> Result<Byte<F>, Error> {
        let p2 = self.mul2(layouter, m2)?;
        let p3 = self.mul3(layouter, m3)?;
        let res = self.xor(layouter, p2, p3)?;
        let res = self.xor(layouter, res, add1)?;
        self.xor(layouter, res, add2)
    }

    // TODO: finish this
    fn mix_column(
        &self,
        layouter: &mut impl Layouter<F>,
        b: [Byte<F>; 4],
    ) -> Result<[Byte<F>; 4], Error> {
        let mut ouputs = vec![];

        ouputs.push(self.mix_row(
            layouter,
            b[0].clone(), //   2 * b0
            b[1].clone(), // + 3 * b1
            b[2].clone(), // + 1 * b2
            b[3].clone(), // + 1 * b3
        )?);
        ouputs.push(self.mix_row(
            layouter,
            b[1].clone(), //   2 * b1
            b[2].clone(), // + 3 * b2
            b[3].clone(), // + 1 * b3
            b[0].clone(), // + 1 * b0
        )?);
        ouputs.push(todo!("fill me in, see https://en.wikipedia.org/wiki/Advanced_Encryption_Standard#The_MixColumns_step"));
        ouputs.push(todo!("fill me in, see https://en.wikipedia.org/wiki/Advanced_Encryption_Standard#The_MixColumns_step"));

        Ok(ouputs.try_into().unwrap())
    }

    fn sub_bytes(
        &self,
        layouter: &mut impl Layouter<F>,
        st: [Byte<F>; 16],
    ) -> Result<[Byte<F>; 16], Error> {
        let mut outputs = vec![];
        for b in st {
            outputs.push(self.sbox(layouter, b)?);
        }
        Ok(outputs.try_into().unwrap())
    }

    // https://en.wikipedia.org/wiki/Advanced_Encryption_Standard#The_ShiftRows_step
    fn shift_rows(
        &self,
        layouter: &mut impl Layouter<F>,
        st: [Byte<F>; 16],
    ) -> Result<[Byte<F>; 16], Error> {
        let mut outputs = vec![];
        outputs.push(st[0].clone());
        outputs.push(st[5].clone());
        outputs.push(st[10].clone());
        outputs.push(st[15].clone());
        outputs.push(st[4].clone());
        outputs.push(st[9].clone());
        outputs.push(st[14].clone());
        outputs.push(st[3].clone());
        outputs.push(st[8].clone());
        outputs.push(st[13].clone());
        outputs.push(st[2].clone());
        outputs.push(st[7].clone());
        outputs.push(st[12].clone());
        outputs.push(st[1].clone());
        outputs.push(st[6].clone());
        outputs.push(st[11].clone());
        Ok(outputs.try_into().unwrap())
    }

    fn mix_columns(
        &self,
        layouter: &mut impl Layouter<F>,
        st: [Byte<F>; 16],
    ) -> Result<[Byte<F>; 16], Error> {
        let mut outputs = vec![];
        for col in 0..4 {
            let b = [
                st[col].clone(),
                st[col + 4].clone(),
                st[col + 8].clone(),
                st[col + 12].clone(),
            ];
            outputs.extend(self.mix_column(layouter, b)?);
        }
        Ok(outputs.try_into().unwrap())
    }

    fn add_round_key(
        &self,
        layouter: &mut impl Layouter<F>,
        st: [Byte<F>; 16],
        round_key: [Byte<F>; 16],
    ) -> Result<[Byte<F>; 16], Error> {
        todo!("xor st and round_key together")
    }

    // https://en.wikipedia.org/wiki/Advanced_Encryption_Standard#High-level_description_of_the_algorithm
    fn aes(
        &self,
        layouter: &mut impl Layouter<F>,
        pt: [Byte<F>; 16],
        round_keys: [[Byte<F>; 16]; 11],
    ) -> Result<[Byte<F>; 16], Error> {
        let mut st = pt;
        let mut keys = round_keys.iter().cloned();

        // Initial round key addition:
        st = self.add_round_key(layouter, st, keys.next().unwrap())?;

        // 9 Regular Rounds:
        for _ in 0..9 {
            st = self.sub_bytes(layouter, st)?;
            st = self.shift_rows(layouter, st)?;
            st = self.mix_columns(layouter, st)?;
            st = self.add_round_key(layouter, st, keys.next().unwrap())?;
        }

        // Final Round:
        st = self.sub_bytes(layouter, st)?;
        st = self.shift_rows(layouter, st)?;
        self.add_round_key(layouter, st, keys.next().unwrap())
    }

    fn aes_expand_key(
        &self,
        layouter: &mut impl Layouter<F>,
        key: [Byte<F>; 16],
    ) -> Result<[[Byte<F>; 16]; 11], Error> {
        todo!("implement the AES key-schedule: https://en.wikipedia.org/wiki/AES_key_schedule")
    }
}

const TYP_XOR: u64 = 2;
const TYP_SBOX: u64 = 1;
const TYP_MUL2: u64 = 3;
const TYP_MUL3: u64 = 4;

const SBOX: [u8; 0x100] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

// for all (a, b):
//   (XOR, a, b, a XOR b) <-- binary operation
//
// for all a:
//   (SBOX, a, 0, sbox(a)) <-- unary operation

#[derive(Clone, Debug)]
struct TestConfig<F: Field + Clone> {
    _ph: PhantomData<F>,
    lookup: LookupChip<F>,
    input1: Column<Advice>,
    input2: Column<Advice>,
    output: Column<Advice>,
}

fn op_mul2(a: u8) -> u8 {
    if a & 0x80 == 0 {
        a << 1
    } else {
        (a << 1) ^ 0x1b
    }
}

fn op_mul3(a: u8) -> u8 {
    a ^ op_mul2(a)
}

impl<F: PrimeField> Circuit<F> for TestCircuit<F> {
    type Config = TestConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        TestCircuit {
            _ph: PhantomData,
            key: Value::unknown(),
            pt: Value::unknown(),
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let fixed = meta.fixed_column();
        let input1 = meta.advice_column();
        let input2 = meta.advice_column();
        let output = meta.advice_column();

        meta.enable_equality(input1);
        meta.enable_equality(input2);
        meta.enable_equality(output);

        let lookup = LookupChip::configure(meta, fixed, input1, input2, output);

        TestConfig {
            _ph: PhantomData,
            lookup,
            input1,
            input2,
            output,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config, //
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // initialize the lookup table
        config.lookup.initialize(&mut layouter)?;

        // load the AES cipher key
        let mut key = Vec::new();
        for i in 0..0x10 {
            let b = config.free(&mut layouter, self.key.map(|k| k[i]))?;
            key.push(b);
        }
        let key: [Byte<F>; 0x10] = key.try_into().unwrap();

        // load the plaintext
        let mut pt = Vec::new();
        for i in 0..0x10 {
            let b = config.free(&mut layouter, self.pt.map(|p| p[i]))?;
            pt.push(b);
        }
        let pt: [Byte<F>; 0x10] = pt.try_into().unwrap();

        // TODO: compute the round keys
        let round_keys: [_; 11] = [
            // TODO: replace this when you implement the key schedule
            key.clone(),
            key.clone(),
            key.clone(),
            key.clone(),
            key.clone(),
            key.clone(),
            key.clone(),
            key.clone(),
            key.clone(),
            key.clone(),
            key.clone(),
        ];

        // perform the AES encryption
        return Ok(()); // TODO: remove this line when you implement the encryption
        let ct = config.lookup.aes(&mut layouter, pt, round_keys);

        // TODO: export the ciphertext as public inputs
        Ok(())
    }
}

fn main() {
    use halo2_proofs::halo2curves::bn256::Fr;

    // run the MockProver
    let circuit = TestCircuit::<Fr> {
        _ph: PhantomData,
        key: Value::known([
            0x10, 0x43, 0x23, 0x45, //
            0x67, 0x89, 0xab, 0xcd, //
            0xef, 0x10, 0x32, 0x54, //
            0x76, 0x98, 0xba, 0xdc, //
        ]),
        pt: Value::known([
            0xde, 0xad, 0xc0, 0xde, //
            0xde, 0xad, 0xc0, 0xde, //
            0xde, 0xad, 0xc0, 0xde, //
            0xde, 0xad, 0xc0, 0xde, //
        ]),
    };
    let prover = MockProver::run(17, &circuit, vec![]).unwrap();
    prover.verify().unwrap();
}
