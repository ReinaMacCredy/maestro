use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Clone)]
enum Value {
    Object(BTreeMap<String, Value>),
    Array(Vec<Value>),
    String(String),
    Number(i64),
    Bool(bool),
    Null,
}

struct Parser<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> Parser<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, index: 0 }
    }
    fn whitespace(&mut self) {
        while self.index < self.bytes.len()
            && matches!(self.bytes[self.index], b' ' | b'\n' | b'\t')
        {
            self.index += 1;
        }
    }
    fn take(&mut self, expected: u8) -> Result<(), String> {
        self.whitespace();
        if self.bytes.get(self.index) != Some(&expected) {
            return Err(format!("expected JSON byte {}", expected));
        }
        self.index += 1;
        Ok(())
    }
    fn value(&mut self) -> Result<Value, String> {
        self.whitespace();
        match self.bytes.get(self.index) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b'-' | b'0'..=b'9') => Ok(Value::Number(self.number()?)),
            Some(b't') => {
                self.literal(b"true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(Value::Bool(false))
            }
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(Value::Null)
            }
            _ => Err("invalid JSON value".to_string()),
        }
    }
    fn literal(&mut self, literal: &[u8]) -> Result<(), String> {
        if self.bytes.get(self.index..self.index + literal.len()) != Some(literal) {
            return Err("invalid JSON literal".to_string());
        }
        self.index += literal.len();
        Ok(())
    }
    fn object(&mut self) -> Result<Value, String> {
        self.take(b'{')?;
        let mut values = BTreeMap::new();
        self.whitespace();
        if self.bytes.get(self.index) == Some(&b'}') {
            self.index += 1;
            return Ok(Value::Object(values));
        }
        loop {
            self.whitespace();
            let key = self.string()?;
            self.take(b':')?;
            let value = self.value()?;
            if values.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate JSON key: {}", key));
            }
            self.whitespace();
            match self.bytes.get(self.index) {
                Some(b',') => self.index += 1,
                Some(b'}') => {
                    self.index += 1;
                    return Ok(Value::Object(values));
                }
                _ => return Err("object delimiter missing".to_string()),
            }
        }
    }
    fn array(&mut self) -> Result<Value, String> {
        self.take(b'[')?;
        let mut values = Vec::new();
        self.whitespace();
        if self.bytes.get(self.index) == Some(&b']') {
            self.index += 1;
            return Ok(Value::Array(values));
        }
        loop {
            values.push(self.value()?);
            self.whitespace();
            match self.bytes.get(self.index) {
                Some(b',') => self.index += 1,
                Some(b']') => {
                    self.index += 1;
                    return Ok(Value::Array(values));
                }
                _ => return Err("array delimiter missing".to_string()),
            }
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.take(b'"')?;
        let mut output = String::new();
        while let Some(&byte) = self.bytes.get(self.index) {
            self.index += 1;
            match byte {
                b'"' => return Ok(output),
                b'\\' => {
                    let escaped = *self.bytes.get(self.index).ok_or("truncated JSON escape")?;
                    self.index += 1;
                    match escaped {
                        b'"' => output.push('"'),
                        b'\\' => output.push('\\'),
                        b'/' => output.push('/'),
                        b'b' => output.push('\u{0008}'),
                        b'f' => output.push('\u{000c}'),
                        b'n' => output.push('\n'),
                        b'r' => output.push('\r'),
                        b't' => output.push('\t'),
                        b'u' => {
                            let hex = self
                                .bytes
                                .get(self.index..self.index + 4)
                                .ok_or("truncated JSON unicode escape")?;
                            let digits =
                                std::str::from_utf8(hex).map_err(|_| "invalid unicode escape")?;
                            let scalar = u32::from_str_radix(digits, 16)
                                .map_err(|_| "invalid unicode escape")?;
                            output.push(char::from_u32(scalar).ok_or("invalid unicode scalar")?);
                            self.index += 4;
                        }
                        _ => return Err("invalid JSON escape".to_string()),
                    }
                }
                0..=31 => return Err("control character in JSON string".to_string()),
                _ => output.push(byte as char),
            }
        }
        Err("unterminated JSON string".to_string())
    }
    fn number(&mut self) -> Result<i64, String> {
        let start = self.index;
        if self.bytes.get(self.index) == Some(&b'-') {
            self.index += 1;
        }
        while matches!(self.bytes.get(self.index), Some(b'0'..=b'9')) {
            self.index += 1;
        }
        std::str::from_utf8(&self.bytes[start..self.index])
            .map_err(|_| "invalid number".to_string())?
            .parse::<i64>()
            .map_err(|_| "invalid number".to_string())
    }
}

fn load(path: &Path) -> Result<Value, String> {
    let raw = fs::read(path).map_err(|error| error.to_string())?;
    if !raw.ends_with(b"\n") || raw.contains(&b'\r') || raw.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(format!("noncanonical JSON: {}", path.display()));
    }
    let mut parser = Parser::new(&raw);
    let value = parser.value()?;
    parser.whitespace();
    if parser.index != raw.len() {
        return Err("trailing JSON bytes".to_string());
    }
    Ok(value)
}

fn object(value: &Value) -> Result<&BTreeMap<String, Value>, String> {
    match value {
        Value::Object(value) => Ok(value),
        _ => Err("JSON object required".to_string()),
    }
}
fn array(value: &Value) -> Result<&Vec<Value>, String> {
    match value {
        Value::Array(value) => Ok(value),
        _ => Err("JSON array required".to_string()),
    }
}
fn string(value: &Value) -> Result<&str, String> {
    match value {
        Value::String(value) => Ok(value),
        _ => Err("JSON string required".to_string()),
    }
}
fn number(value: &Value) -> Result<i64, String> {
    match value {
        Value::Number(value) => Ok(*value),
        _ => Err("JSON number required".to_string()),
    }
}
fn field<'a>(value: &'a BTreeMap<String, Value>, name: &str) -> Result<&'a Value, String> {
    value
        .get(name)
        .ok_or_else(|| format!("missing JSON field: {}", name))
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(data: &[u8]) -> String {
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() + 8) % 64 != 0 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in padded.chunks(64) {
        let mut w = [0u32; 64];
        for index in 0..16 {
            w[index] = u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    format!(
        "sha256:{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]
    )
}

fn quote(value: &str) -> String {
    format!("{:?}", value)
}
fn run(argv: &Vec<Value>, cwd: &str) -> Result<i64, String> {
    let program = string(argv.first().ok_or("empty argv")?)?;
    let args: Result<Vec<&str>, String> = argv.iter().skip(1).map(string).collect();
    let status = Command::new(program)
        .args(args?)
        .current_dir(cwd)
        .status()
        .map_err(|error| error.to_string())?;
    Ok(status.code().unwrap_or(-1) as i64)
}

fn main_result() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("expected snapshot ledger readback source output".to_string());
    }
    let snapshot_path = Path::new(&args[0]);
    let ledger_path = Path::new(&args[1]);
    let readback_path = Path::new(&args[2]);
    let snapshot = object(&load(snapshot_path)?)?;
    let ledger = object(&load(ledger_path)?)?;
    let readback = object(&load(readback_path)?)?;
    if string(field(snapshot, "schema_version")?)?
        != "maestro.external.vnext-final-cumulative-closure-snapshot.v1"
    {
        return Err("snapshot schema differs".to_string());
    }
    if string(field(ledger, "schema_version")?)? != "maestro.external.vnext-final-proof-ledger.v1" {
        return Err("ledger schema differs".to_string());
    }
    if string(field(readback, "schema_version")?)?
        != "maestro.external.vnext-stage12-semantic-readback-plan.v1"
    {
        return Err("readback schema differs".to_string());
    }
    let ledger_proofs = array(field(ledger, "proofs")?)?;
    let mut proof_ids = BTreeSet::new();
    let mut stages = BTreeSet::new();
    for proof in ledger_proofs {
        let proof = object(proof)?;
        if !proof_ids.insert(string(field(proof, "proof_id")?)?.to_string()) {
            return Err("duplicate proof id".to_string());
        }
        stages.insert(number(field(proof, "stage")?)?);
        let engines: Result<BTreeSet<String>, String> = array(field(proof, "engines")?)?
            .iter()
            .map(|value| string(value).map(str::to_string))
            .collect();
        if engines?
            != ["python".to_string(), "ruby".to_string(), "rust".to_string()]
                .into_iter()
                .collect()
        {
            return Err("ledger engine coverage differs".to_string());
        }
    }
    if stages != (0..13).collect() {
        return Err("ledger stage closure differs".to_string());
    }
    let required_readback: BTreeSet<String> = [
        "compiled_namespace_absence",
        "generated_resource_absence",
        "persisted_identity_parity",
        "canonical_facade_behavior",
        "migration_route_absence",
        "retained_reader_absence",
        "consumer_reader_hold_zero",
        "negative_fixture",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    let actual_readback: Result<BTreeSet<String>, String> = array(field(readback, "checks")?)?
        .iter()
        .map(|value| {
            object(value)
                .and_then(|row| field(row, "kind"))
                .and_then(string)
                .map(str::to_string)
        })
        .collect();
    if actual_readback? != required_readback {
        return Err("semantic readback closure differs".to_string());
    }
    let mut proof_rows = Vec::new();
    for proof in ledger_proofs {
        let proof = object(proof)?;
        let id = string(field(proof, "proof_id")?)?;
        let expected = string(field(proof, "expected_outcome")?)?;
        let command = object(field(proof, "command")?)?;
        let exit = run(array(field(command, "argv")?)?, &args[3])?;
        let actual = if exit == number(field(command, "expected_exit_code")?)? {
            expected
        } else {
            "error"
        };
        proof_rows.push(format!(
            "{{\"actual_outcome\":{},\"expected_outcome\":{},\"exit_code\":{},\"proof_id\":{}}}",
            quote(actual),
            quote(expected),
            exit,
            quote(id)
        ));
    }
    let mut checks = Vec::new();
    let mut all_pass = true;
    for check in array(field(readback, "checks")?)? {
        let check = object(check)?;
        let exit = run(array(field(check, "argv")?)?, &args[3])?;
        let passed = exit == number(field(check, "expected_exit_code")?)?;
        all_pass &= passed;
        checks.push(format!(
            "{{\"exit_code\":{},\"id\":{},\"kind\":{},\"status\":{}}}",
            exit,
            quote(string(field(check, "id")?)?),
            quote(string(field(check, "kind")?)?),
            quote(if passed { "pass" } else { "fail" })
        ));
    }
    let snapshot_identity = sha256(&fs::read(snapshot_path).map_err(|error| error.to_string())?);
    let ledger_identity = sha256(&fs::read(ledger_path).map_err(|error| error.to_string())?);
    let receipt = format!("{{\"engine\":\"rust\",\"ledger_identity\":{},\"proofs\":[{}],\"schema_version\":\"maestro.external.vnext-final-engine-receipt.v1\",\"semantic_readback\":{{\"checks\":[{}],\"status\":{}}},\"snapshot_identity\":{}}}\n", quote(&ledger_identity), proof_rows.join(","), checks.join(","), quote(if all_pass {"pass"} else {"fail"}), quote(&snapshot_identity));
    fs::write(&args[4], receipt).map_err(|error| error.to_string())
}

fn main() {
    if let Err(error) = main_result() {
        eprintln!("{}", error);
        std::process::exit(1);
    }
}
