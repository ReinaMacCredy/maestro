use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
fn run(
    argv: &Vec<Value>,
    cwd: &str,
    tools: &BTreeMap<String, String>,
    snapshot: &str,
    packet: &str,
    environment: &BTreeMap<String, String>,
) -> Result<Output, String> {
    let program = string(argv.first().ok_or("empty argv")?)?;
    let expand = |value: &str| -> Result<String, String> {
        if value == "{source}" {
            Ok(cwd.to_string())
        } else if value == "{control:ancestry-repository}" {
            Ok(Path::new(snapshot)
                .parent()
                .ok_or("snapshot has no control root")?
                .join("ancestry-repository")
                .to_string_lossy()
                .to_string())
        } else if value == "{control:snapshot}" {
            Ok(snapshot.to_string())
        } else if value == "{packet:fanout-manifest.v4.json}" {
            Ok(Path::new(packet)
                .join("fanout-manifest.v4.json")
                .to_string_lossy()
                .to_string())
        } else if value.starts_with("{tool:") && value.ends_with('}') {
            tools
                .get(&value[6..value.len() - 1])
                .cloned()
                .ok_or_else(|| format!("unknown frozen tool: {}", value))
        } else if value.contains('{') || value.contains('}') {
            Err(format!("unknown command placeholder: {}", value))
        } else {
            Ok(value.to_string())
        }
    };
    let executable = expand(program)?;
    let args: Result<Vec<String>, String> = argv
        .iter()
        .skip(1)
        .map(|value| string(value).and_then(expand))
        .collect();
    Command::new(executable)
        .args(args?)
        .current_dir(cwd)
        .envs(environment)
        .output()
        .map_err(|error| error.to_string())
}

fn safe_relative(root: &Path, value: &str) -> Result<PathBuf, String> {
    let relative = Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(format!("unsafe relative path: {}", value));
    }
    Ok(root.join(relative))
}

fn verify_binding(root: &Path, binding: &BTreeMap<String, Value>) -> Result<(), String> {
    let path = safe_relative(root, string(field(binding, "path")?)?)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("bound file absent or unsafe: {}", path.display()));
    }
    let raw = fs::read(&path).map_err(|error| error.to_string())?;
    if number(field(binding, "byte_length")?)? != raw.len() as i64
        || string(field(binding, "sha256")?)? != sha256(&raw)
    {
        return Err(format!("bound file differs: {}", path.display()));
    }
    Ok(())
}

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!("source symlink is forbidden: {}", path.display()));
        }
        if metadata.is_dir() {
            collect_files(root, &path, output)?;
        } else if metadata.is_file() {
            output.push(
                path.strip_prefix(root)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

fn stream_json(raw: &[u8]) -> String {
    format!(
        "{{\"byte_length\":{},\"sha256\":{}}}",
        raw.len(),
        quote(&sha256(raw))
    )
}

fn command_identity(command: &BTreeMap<String, Value>) -> Result<String, String> {
    let argv: Result<Vec<String>, String> = array(field(command, "argv")?)?
        .iter()
        .map(|value| string(value).map(quote))
        .collect();
    let canonical = format!(
        "{{\"argv\":[{}],\"expected_exit_code\":{}}}\n",
        argv?.join(","),
        number(field(command, "expected_exit_code")?)?
    );
    Ok(sha256(canonical.as_bytes()))
}

fn main_result() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 8 {
        return Err(
            "expected snapshot manifest ledger readback toolchain packet source output".to_string(),
        );
    }
    let snapshot_path = Path::new(&args[0]);
    let manifest_path = Path::new(&args[1]);
    let ledger_path = Path::new(&args[2]);
    let readback_path = Path::new(&args[3]);
    let toolchain_path = Path::new(&args[4]);
    let packet_path = Path::new(&args[5]);
    let source_path = Path::new(&args[6]);
    let frozen_root = snapshot_path
        .parent()
        .and_then(Path::parent)
        .ok_or("snapshot has no frozen root")?;
    let snapshot_value = load(snapshot_path)?;
    let manifest_value = load(manifest_path)?;
    let ledger_value = load(ledger_path)?;
    let readback_value = load(readback_path)?;
    let toolchain_value = load(toolchain_path)?;
    let packet_manifest_path = packet_path.join("packet-manifest.v1.json");
    let packet_manifest_value = load(&packet_manifest_path)?;
    let snapshot = object(&snapshot_value)?;
    let manifest = object(&manifest_value)?;
    let ledger = object(&ledger_value)?;
    let readback = object(&readback_value)?;
    let toolchain = object(&toolchain_value)?;
    let packet_manifest = object(&packet_manifest_value)?;
    let registry_binding = object(field(snapshot, "proof_registry")?)?;
    verify_binding(source_path, registry_binding)?;
    let registry_path = safe_relative(source_path, string(field(registry_binding, "path")?)?)?;
    let registry_value = load(&registry_path)?;
    let registry = object(&registry_value)?;
    if string(field(registry, "schema_version")?)?
        != "maestro.external.vnext-final-proof-registry.v1"
        || string(field(registry, "registry_identity_policy")?)?
            != "canonical-bytes-bound-no-inference-no-reassignment"
    {
        return Err("normative proof registry differs".to_string());
    }
    if string(field(snapshot, "schema_version")?)?
        != "maestro.external.vnext-final-cumulative-closure-snapshot.v1"
    {
        return Err("snapshot schema differs".to_string());
    }
    if string(field(snapshot, "state")?)? != "frozen"
        || string(field(snapshot, "approved_packet_identity")?)?
            != "sha256:2026513c84b1993f020f7d0430154ec0bc4e821438ccefd7dd6b91834a3d6283"
    {
        return Err("snapshot state or packet identity differs".to_string());
    }
    let environment_allowlist: Result<Vec<String>, String> =
        array(field(snapshot, "environment_allowlist")?)?
            .iter()
            .map(|value| string(value).map(str::to_string))
            .collect();
    if environment_allowlist? != ["HOME", "LANG", "LC_ALL", "PATH", "TMPDIR", "TZ"] {
        return Err("environment allowlist differs".to_string());
    }
    let immutable_roots: Result<Vec<String>, String> =
        array(field(snapshot, "immutable_input_roots")?)?
            .iter()
            .map(|value| string(value).map(str::to_string))
            .collect();
    if immutable_roots? != ["source", "packet", "control", "dependencies"] {
        return Err("immutable roots differ".to_string());
    }
    if string(field(snapshot, "cache_policy")?)?
        != "immutable_compilation_and_dependency_bytes_only"
        || object(field(snapshot, "pointer_preimage")?).is_err()
    {
        return Err("cache policy or pointer preimage differs".to_string());
    }
    let publication = object(field(snapshot, "publication_root_identity")?)?;
    let expected_generation = number(field(snapshot, "expected_generation")?)?;
    if publication.keys().cloned().collect::<BTreeSet<_>>()
        != ["path", "device", "inode", "mount_device"]
            .into_iter()
            .map(str::to_string)
            .collect()
        || expected_generation < 0
        || number(field(
            object(field(snapshot, "pointer_preimage")?)?,
            "generation",
        )?)? != expected_generation
    {
        return Err("publication custody or generation differs".to_string());
    }
    let denials: Result<BTreeSet<String>, String> = array(field(snapshot, "effect_denylist")?)?
        .iter()
        .map(|value| string(value).map(str::to_string))
        .collect();
    let denials = denials?;
    if [
        "network",
        "protected_primary_checkout_write",
        "outside_packet_bound_roots_write",
    ]
    .iter()
    .any(|value| !denials.contains(*value))
    {
        return Err("effect denylist differs".to_string());
    }
    if string(field(ledger, "schema_version")?)? != "maestro.external.vnext-final-proof-ledger.v1" {
        return Err("ledger schema differs".to_string());
    }
    if string(field(readback, "schema_version")?)?
        != "maestro.external.vnext-stage12-semantic-readback-plan.v1"
    {
        return Err("readback schema differs".to_string());
    }
    if string(field(manifest, "schema_version")?)?
        != "maestro.external.vnext-final-input-manifest.v1"
    {
        return Err("input manifest schema differs".to_string());
    }
    if string(field(toolchain, "schema_version")?)? != "maestro.external.vnext-final-toolchain.v1" {
        return Err("toolchain schema differs".to_string());
    }
    let packet_binding = object(field(snapshot, "packet_manifest")?)?;
    if string(field(packet_binding, "path")?)? != "packet/packet-manifest.v1.json" {
        return Err("packet-manifest binding differs".to_string());
    }
    verify_binding(
        packet_path.parent().ok_or("packet root has no parent")?,
        packet_binding,
    )?;
    if string(field(packet_manifest, "schema_version")?)?
        != "maestro.external.vnext-final-packet-manifest.v1"
    {
        return Err("packet manifest schema differs".to_string());
    }
    if string(field(packet_manifest, "approved_packet_identity")?)?
        != string(field(snapshot, "approved_packet_identity")?)?
    {
        return Err("packet identity differs".to_string());
    }
    let packet_files = array(field(packet_manifest, "files")?)?;
    let mut expected_packet_paths = BTreeSet::new();
    let mut packet_bytes = 0i64;
    for row in packet_files {
        let row = object(row)?;
        let relative = string(field(row, "path")?)?
            .strip_prefix("packet/")
            .ok_or("packet path prefix differs")?;
        if !expected_packet_paths.insert(relative.to_string()) {
            return Err("packet manifest duplicates a path".to_string());
        }
        let adjusted = BTreeMap::from([
            ("path".to_string(), Value::String(relative.to_string())),
            (
                "byte_length".to_string(),
                field(row, "byte_length")?.clone(),
            ),
            ("sha256".to_string(), field(row, "sha256")?.clone()),
        ]);
        verify_binding(packet_path, &adjusted)?;
        packet_bytes += number(field(row, "byte_length")?)?;
    }
    let mut actual_packet_paths = Vec::new();
    collect_files(packet_path, packet_path, &mut actual_packet_paths)?;
    actual_packet_paths.retain(|path| path != "packet-manifest.v1.json");
    actual_packet_paths.sort();
    if actual_packet_paths != expected_packet_paths.iter().cloned().collect::<Vec<_>>() {
        return Err("packet manifest has an omission".to_string());
    }
    if number(field(packet_manifest, "file_count")?)? != packet_files.len() as i64
        || number(field(packet_manifest, "byte_length")?)? != packet_bytes
    {
        return Err("packet manifest totals differ".to_string());
    }
    if string(field(toolchain, "target")?)?.is_empty()
        || string(field(toolchain, "profile")?)?.is_empty()
    {
        return Err("toolchain target or profile is absent".to_string());
    }
    let environment = object(field(toolchain, "environment")?)?;
    let expected_environment: BTreeMap<String, String> = [
        ("LANG".to_string(), "C".to_string()),
        ("LC_ALL".to_string(), "C".to_string()),
        ("TZ".to_string(), "UTC".to_string()),
    ]
    .into_iter()
    .collect();
    let actual_environment: Result<BTreeMap<String, String>, String> = environment
        .iter()
        .map(|(key, value)| Ok((key.clone(), string(value)?.to_string())))
        .collect();
    if actual_environment? != expected_environment {
        return Err("toolchain environment differs".to_string());
    }
    let lockfiles = array(field(toolchain, "lockfiles")?)?;
    if lockfiles.is_empty() {
        return Err("lockfile closure is absent".to_string());
    }
    for lockfile in lockfiles {
        verify_binding(source_path, object(lockfile)?)?;
    }
    let stages = array(field(snapshot, "first_parent_stages")?)?;
    if stages.len() != 13 {
        return Err("Stage checkpoint closure differs".to_string());
    }
    for (expected, row) in stages.iter().enumerate() {
        let row = object(row)?;
        if number(field(row, "stage")?)? != expected as i64 {
            return Err("Stage checkpoint order differs".to_string());
        }
        let checkpoint_binding = object(field(row, "checkpoint")?)?;
        verify_binding(frozen_root, checkpoint_binding)?;
        let checkpoint_path =
            safe_relative(frozen_root, string(field(checkpoint_binding, "path")?)?)?;
        let checkpoint_value = load(&checkpoint_path)?;
        let checkpoint = object(&checkpoint_value)?;
        for field_name in ["stage", "commit", "tree"] {
            let matches = match field_name {
                "stage" => {
                    number(field(checkpoint, field_name)?)? == number(field(row, field_name)?)?
                }
                _ => string(field(checkpoint, field_name)?)? == string(field(row, field_name)?)?,
            };
            if !matches {
                return Err("Stage checkpoint bytes differ".to_string());
            }
        }
        let checkpoint_parents: Result<Vec<String>, String> = array(field(checkpoint, "parents")?)?
            .iter()
            .map(|value| string(value).map(str::to_string))
            .collect();
        let row_parents: Result<Vec<String>, String> = array(field(row, "parents")?)?
            .iter()
            .map(|value| string(value).map(str::to_string))
            .collect();
        if checkpoint_parents? != row_parents? {
            return Err("Stage checkpoint parent bytes differ".to_string());
        }
    }
    let historical = [
        "66a34db6a28ff3f3ee178f644b645bb6ea60681e",
        "bebc2e35314741c3b053901fd5040b323ee2c924",
        "602302c69df22e96f319b1451c14d341fdde14cd",
        "ad8f3d88bf647031d415aa3aed0998ca7a9097d7",
        "9f3cc73b2199c5b2be78dcea8852cbdcafaaafc2",
    ];
    for (stage, expected) in historical.iter().enumerate() {
        if string(field(object(&stages[stage])?, "commit")?)? != *expected {
            return Err("historical Stage 0-4 checkpoints differ".to_string());
        }
    }
    let stage5 = object(&stages[5])?;
    let stage5_parents = array(field(stage5, "parents")?)?;
    if stage5_parents.len() != 2
        || string(&stage5_parents[0])? != string(field(object(&stages[4])?, "commit")?)?
    {
        return Err("Stage 5 merge parent topology differs".to_string());
    }
    for stage in 6..12 {
        let parents = array(field(object(&stages[stage])?, "parents")?)?;
        if parents.len() != 1
            || string(&parents[0])? != string(field(object(&stages[stage - 1])?, "commit")?)?
        {
            return Err("Stage 6-11 direct-parent topology differs".to_string());
        }
    }
    let reviewed = object(field(snapshot, "stage12_reviewed_candidate")?)?;
    let stage12 = object(&stages[12])?;
    let stage12_parents = array(field(stage12, "parents")?)?;
    if stage12_parents.len() != 2
        || string(&stage12_parents[0])? != string(field(object(&stages[11])?, "commit")?)?
        || string(&stage12_parents[1])? != string(field(reviewed, "commit")?)?
        || string(field(stage12, "tree")?)? != string(field(reviewed, "tree")?)?
    {
        return Err("Stage 12 reviewed-candidate merge topology differs".to_string());
    }
    let overlay_binding = object(field(snapshot, "stage12_overlay")?)?;
    verify_binding(frozen_root, overlay_binding)?;
    let overlay_path = safe_relative(frozen_root, string(field(overlay_binding, "path")?)?)?;
    let overlay_value = load(&overlay_path)?;
    let overlay = object(&overlay_value)?;
    if string(field(overlay, "schema_version")?)?
        != "maestro.external.vnext-final-stage12-overlay.v1"
        || string(field(overlay, "stage11_commit")?)?
            != string(field(object(&stages[11])?, "commit")?)?
        || string(field(overlay, "reviewed_candidate_commit")?)?
            != string(field(reviewed, "commit")?)?
        || string(field(overlay, "stage12_commit")?)? != string(field(stage12, "commit")?)?
    {
        return Err("Stage 12 overlay binding differs".to_string());
    }
    let promotion_binding = object(field(snapshot, "promotion_prerequisites")?)?;
    verify_binding(frozen_root, promotion_binding)?;
    let promotion_path = safe_relative(frozen_root, string(field(promotion_binding, "path")?)?)?;
    let promotion_value = load(&promotion_path)?;
    let promotion = object(&promotion_value)?;
    let legacy_gate = object(field(promotion, "legacy_prune_gate")?)?;
    let consumer_hold = object(field(promotion, "consumer_reader_hold")?)?;
    let parity = object(field(promotion, "promotion_parity")?)?;
    if string(field(promotion, "schema_version")?)?
        != "maestro.external.vnext-final-promotion-prerequisites.v1"
        || string(field(promotion, "stage11_commit")?)?
            != string(field(object(&stages[11])?, "commit")?)?
        || string(field(promotion, "stage12_reviewed_candidate")?)?
            != string(field(reviewed, "commit")?)?
        || number(field(legacy_gate, "observed_legacy_row_count")?)? != 0
        || number(field(consumer_hold, "consumer_count")?)? != 0
        || number(field(consumer_hold, "reader_count")?)? != 0
        || number(field(consumer_hold, "hold_count")?)? != 0
        || number(field(parity, "source_file_count")?)? != 210
        || number(field(parity, "promoted_file_count")?)? != 210
        || number(field(parity, "mismatch_count")?)? != 0
    {
        return Err("promotion prerequisites are absent or nonzero".to_string());
    }
    let promotion_root = promotion_path
        .parent()
        .ok_or("promotion prerequisite has no parent")?;
    for section in [legacy_gate, consumer_hold, parity] {
        verify_binding(promotion_root, object(field(section, "receipt")?)?)?;
    }
    let final_integration = object(field(snapshot, "final_integration")?)?;
    if string(field(
        object(stages.last().ok_or("Stage 12 absent")?)?,
        "commit",
    )?)? != string(field(final_integration, "commit")?)?
    {
        return Err("current V4 Stage 12 checkpoint differs".to_string());
    }
    let roles = array(field(snapshot, "writable_root_roles")?)?;
    let unique_roles: Result<BTreeSet<String>, String> = roles
        .iter()
        .map(|value| string(value).map(str::to_string))
        .collect();
    if roles.len() != 12 || unique_roles?.len() != 12 {
        return Err("disjoint writable roots differ".to_string());
    }
    if string(field(snapshot, "sandbox_profile")?)? != "macos-sandbox-exec-no-network-v1" {
        return Err("sandbox profile differs".to_string());
    }
    let engines: Result<Vec<String>, String> = array(field(snapshot, "engines")?)?
        .iter()
        .map(|value| {
            object(value)
                .and_then(|row| field(row, "id"))
                .and_then(string)
                .map(str::to_string)
        })
        .collect();
    if engines? != ["python", "rust", "ruby"] {
        return Err("engine closure differs".to_string());
    }
    for row in array(field(snapshot, "engines")?)? {
        verify_binding(source_path, object(field(object(row)?, "source")?)?)?;
    }
    for field_name in [
        "input_manifest",
        "proof_ledger",
        "stage12_readback",
        "toolchain",
        "stage12_overlay",
        "ancestry_pack",
        "promotion_prerequisites",
    ] {
        verify_binding(frozen_root, object(field(snapshot, field_name)?)?)?;
    }
    let manifest_rows = array(field(manifest, "entries")?)?;
    let mut manifest_paths = BTreeSet::new();
    let mut manifest_bytes = 0i64;
    for row in manifest_rows {
        let row = object(row)?;
        let path = string(field(row, "path")?)?.to_string();
        if !manifest_paths.insert(path) {
            return Err("input manifest duplicates a path".to_string());
        }
        manifest_bytes += number(field(row, "byte_length")?)?;
        verify_binding(source_path, row)?;
    }
    let mut actual_paths = Vec::new();
    collect_files(source_path, source_path, &mut actual_paths)?;
    actual_paths.sort();
    if actual_paths != manifest_paths.iter().cloned().collect::<Vec<_>>() {
        return Err("input manifest has an omission or extra path".to_string());
    }
    if number(field(manifest, "entry_count")?)? != manifest_rows.len() as i64
        || number(field(manifest, "byte_length")?)? != manifest_bytes
    {
        return Err("input manifest totals differ".to_string());
    }
    let tool_rows = object(field(toolchain, "tools")?)?;
    let expected_tools: BTreeSet<String> = ["python", "rust", "ruby", "cargo", "git"]
        .into_iter()
        .map(str::to_string)
        .collect();
    if tool_rows.keys().cloned().collect::<BTreeSet<_>>() != expected_tools {
        return Err("toolchain closure differs".to_string());
    }
    let mut tools = BTreeMap::new();
    for (name, value) in tool_rows {
        let row = object(value)?;
        let path = string(field(row, "resolved_path")?)?.to_string();
        let raw = fs::read(&path).map_err(|error| error.to_string())?;
        if number(field(row, "byte_length")?)? != raw.len() as i64
            || string(field(row, "sha256")?)? != sha256(&raw)
        {
            return Err(format!("tool bytes differ: {}", name));
        }
        let probe = array(field(row, "probe_argv")?)?;
        let program = string(probe.first().ok_or("tool probe is empty")?)?;
        let probe_args: Result<Vec<&str>, String> = probe.iter().skip(1).map(string).collect();
        let output = Command::new(program)
            .args(probe_args?)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.code().unwrap_or(-1) as i64 != number(field(row, "probe_exit_code")?)? {
            return Err(format!("tool probe exit differs: {}", name));
        }
        for (label, raw) in [
            ("probe_stdout", output.stdout.as_slice()),
            ("probe_stderr", output.stderr.as_slice()),
        ] {
            let expected = object(field(row, label)?)?;
            if number(field(expected, "byte_length")?)? != raw.len() as i64
                || string(field(expected, "sha256")?)? != sha256(raw)
            {
                return Err(format!("tool probe bytes differ: {}", name));
            }
        }
        tools.insert(name.clone(), path);
    }
    let dependencies = array(field(toolchain, "dependency_outputs")?)?;
    let dependency_names: Result<BTreeSet<String>, String> = dependencies
        .iter()
        .map(|value| {
            object(value)
                .and_then(|row| field(row, "name"))
                .and_then(string)
                .map(str::to_string)
        })
        .collect();
    let expected_dependency_names: BTreeSet<String> = [
        "python-complete-cargo-native-closure",
        "rust-complete-cargo-native-closure",
        "ruby-complete-cargo-native-closure",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    if dependencies.len() != 3 || dependency_names? != expected_dependency_names {
        return Err("dependency-output closure is absent".to_string());
    }
    for dependency in dependencies {
        let dependency = object(dependency)?;
        let root = Path::new(string(field(dependency, "resolved_path")?)?);
        let metadata = fs::symlink_metadata(root).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("dependency-output root is absent or unsafe".to_string());
        }
        let files = array(field(dependency, "files")?)?;
        if files.is_empty() {
            return Err("dependency-output file closure is empty".to_string());
        }
        let mut expected_paths = BTreeSet::new();
        let mut canonical_rows = Vec::new();
        let mut total = 0i64;
        for row in files {
            let row = object(row)?;
            let relative = string(field(row, "path")?)?;
            if !expected_paths.insert(relative.to_string()) {
                return Err("dependency-output manifest duplicates a path".to_string());
            }
            verify_binding(root, row)?;
            let bytes = number(field(row, "byte_length")?)?;
            total += bytes;
            canonical_rows.push(format!(
                "{{\"byte_length\":{},\"path\":{},\"sha256\":{}}}",
                bytes,
                quote(relative),
                quote(string(field(row, "sha256")?)?)
            ));
        }
        let mut actual_paths = Vec::new();
        collect_files(root, root, &mut actual_paths)?;
        actual_paths.sort();
        if actual_paths != expected_paths.iter().cloned().collect::<Vec<_>>() {
            return Err("dependency-output manifest has an omission".to_string());
        }
        let canonical = format!("[{}]\n", canonical_rows.join(","));
        if number(field(dependency, "file_count")?)? != files.len() as i64
            || number(field(dependency, "byte_length")?)? != total
            || string(field(dependency, "identity")?)? != sha256(canonical.as_bytes())
        {
            return Err("dependency-output identity differs".to_string());
        }
        let probe = object(field(dependency, "completeness_probe")?)?;
        let probe_argv: Result<BTreeSet<String>, String> = array(field(probe, "argv")?)?
            .iter()
            .map(|value| string(value).map(str::to_string))
            .collect();
        let required_probe: BTreeSet<String> =
            ["fetch", "--offline", "--frozen", "--locked", "--target"]
                .into_iter()
                .map(str::to_string)
                .collect();
        if number(field(probe, "exit_code")?)? != 0 || !required_probe.is_subset(&probe_argv?) {
            return Err("dependency closure completeness probe is absent".to_string());
        }
    }
    let ledger_proofs = array(field(ledger, "proofs")?)?;
    if number(field(ledger, "proof_count")?)? != ledger_proofs.len() as i64 {
        return Err("ledger count differs".to_string());
    }
    let ledger_registry = object(field(ledger, "registry_identity")?)?;
    for name in ["path", "sha256"] {
        if string(field(ledger_registry, name)?)? != string(field(registry_binding, name)?)? {
            return Err("ledger binds another proof registry".to_string());
        }
    }
    if number(field(ledger_registry, "byte_length")?)?
        != number(field(registry_binding, "byte_length")?)?
    {
        return Err("ledger binds another proof registry".to_string());
    }
    let mut normative = BTreeMap::new();
    for row in array(field(registry, "proofs")?)? {
        let row = object(row)?;
        let id = string(field(row, "proof_id")?)?;
        if normative.insert(id.to_string(), row).is_some() {
            return Err("registry duplicates a proof id".to_string());
        }
    }
    let mut proof_ids = BTreeSet::new();
    let mut stages = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for proof in ledger_proofs {
        let proof = object(proof)?;
        if !proof_ids.insert(string(field(proof, "proof_id")?)?.to_string()) {
            return Err("duplicate proof id".to_string());
        }
        stages.insert(number(field(proof, "stage")?)?);
        kinds.insert(string(field(proof, "kind")?)?.to_string());
        let expected = normative
            .get(string(field(proof, "proof_id")?)?)
            .ok_or("ledger proof is absent from normative registry")?;
        for name in ["kind", "expected_outcome"] {
            if string(field(proof, name)?)? != string(field(expected, name)?)? {
                return Err("proof registry classification differs".to_string());
            }
        }
        if number(field(proof, "stage")?)? != number(field(expected, "stage")?)? {
            return Err("proof registry Stage differs".to_string());
        }
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
        for binding in array(field(proof, "input_bindings")?)? {
            verify_binding(source_path, object(binding)?)?;
        }
        let command = object(field(proof, "command")?)?;
        if string(field(command, "identity")?)? != command_identity(command)? {
            return Err("proof command identity differs".to_string());
        }
        let expected_command = object(field(expected, "command")?)?;
        if command_identity(command)? != command_identity(expected_command)? {
            return Err("proof registry command differs".to_string());
        }
        let harness = object(field(proof, "harness")?)?;
        let expected_harness = object(field(expected, "harness")?)?;
        if string(field(harness, "protocol")?)? != string(field(expected_harness, "protocol")?)?
            || string(field(harness, "required_receipt")?)?
                != string(field(expected_harness, "required_receipt")?)?
        {
            return Err("proof registry harness differs".to_string());
        }
        let protocol = string(field(harness, "protocol")?)?;
        if protocol == "fault-observation-v1" {
            verify_binding(source_path, object(field(harness, "fault_schedule")?)?)?;
        }
        if protocol == "cohort-observation-v1" {
            verify_binding(source_path, object(field(harness, "cohort")?)?)?;
        }
    }
    if proof_ids.len() != normative.len() || stages != (0..13).collect() || kinds.len() != 14 {
        return Err("ledger Stage or kind closure differs".to_string());
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
        let harness = object(field(proof, "harness")?)?;
        let receipt_path = Path::new(&args[7])
            .parent()
            .ok_or("engine output has no parent")?
            .join(format!(
                "{}-{}",
                id,
                string(field(harness, "required_receipt")?)?
            ));
        let mut environment = BTreeMap::from([
            ("MAESTRO_FINAL_PROOF_ID".to_string(), id.to_string()),
            (
                "MAESTRO_FINAL_PROOF_RECEIPT".to_string(),
                receipt_path.to_string_lossy().to_string(),
            ),
        ]);
        let kind = string(field(proof, "kind")?)?;
        let protocol = string(field(harness, "protocol")?)?;
        if protocol == "fault-observation-v1" {
            let binding = object(field(harness, "fault_schedule")?)?;
            let path = safe_relative(source_path, string(field(binding, "path")?)?)?;
            environment.insert(
                "MAESTRO_FAULT_SCHEDULE_PATH".to_string(),
                path.to_string_lossy().to_string(),
            );
        }
        if protocol == "cohort-observation-v1" {
            let binding = object(field(harness, "cohort")?)?;
            let path = safe_relative(source_path, string(field(binding, "path")?)?)?;
            environment.insert(
                "MAESTRO_MIGRATION_COHORT_PATH".to_string(),
                path.to_string_lossy().to_string(),
            );
        }
        let mut argv = array(field(command, "argv")?)?.clone();
        if kind == "ancestry" {
            argv.push(Value::String("--output".to_string()));
            argv.push(Value::String(receipt_path.to_string_lossy().to_string()));
        }
        let output = run(&argv, &args[6], &tools, &args[0], &args[5], &environment)?;
        let exit = output.status.code().unwrap_or(-1) as i64;
        let actual = if exit == number(field(command, "expected_exit_code")?)? {
            expected
        } else {
            "error"
        };
        let mut exact_inputs = String::new();
        if protocol == "fault-observation-v1" {
            let schedule_binding = object(field(harness, "fault_schedule")?)?;
            let schedule_path =
                safe_relative(source_path, string(field(schedule_binding, "path")?)?)?;
            let schedule_value = load(&schedule_path)?;
            let schedules = array(field(object(&schedule_value)?, "schedules")?)?;
            let mode = if kind == "race" {
                "race"
            } else {
                "crash_replay"
            };
            let matching: Vec<&Value> = schedules
                .iter()
                .filter(|value| {
                    object(value)
                        .and_then(|row| field(row, "mode"))
                        .and_then(string)
                        .is_ok_and(|value| value == mode)
                })
                .collect();
            if matching.len() != 1 {
                return Err("fault schedule differs".to_string());
            }
            let observation_value = load(&receipt_path)?;
            let observation = object(&observation_value)?;
            let expected_points: Result<Vec<&str>, String> =
                array(field(object(matching[0])?, "points")?)?
                    .iter()
                    .map(string)
                    .collect();
            let observed_points: Result<Vec<&str>, String> =
                array(field(observation, "observed_reached_points")?)?
                    .iter()
                    .map(string)
                    .collect();
            let point_receipts = array(field(observation, "point_receipts")?)?;
            if string(field(observation, "schema_version")?)?
                != "maestro.external.vnext-final-fault-observation.v1"
                || string(field(observation, "proof_id")?)? != id
                || string(field(observation, "schedule_identity")?)?
                    != string(field(schedule_binding, "sha256")?)?
                || observed_points? != expected_points?
                || point_receipts.len() != array(field(object(matching[0])?, "points")?)?.len()
            {
                return Err("fault harness did not emit exact observed reached points".to_string());
            }
            let proof_output = Path::new(&args[7])
                .parent()
                .ok_or("engine output has no parent")?;
            for (sequence, point_receipt) in point_receipts.iter().enumerate() {
                let point_receipt = object(point_receipt)?;
                let point = string(field(point_receipt, "point")?)?;
                let expected_point =
                    string(&array(field(object(matching[0])?, "points")?)?[sequence])?;
                if point != expected_point {
                    return Err("fault point receipt order differs".to_string());
                }
                verify_binding(proof_output, point_receipt)?;
                let event_path =
                    safe_relative(proof_output, string(field(point_receipt, "path")?)?)?;
                let event_value = load(&event_path)?;
                let event = object(&event_value)?;
                if event.len() != 5
                    || string(field(event, "schema_version")?)?
                        != "maestro.external.vnext-final-fault-point-observation.v1"
                    || string(field(event, "proof_id")?)? != id
                    || string(field(event, "point")?)? != point
                    || number(field(event, "sequence")?)? != sequence as i64
                    || string(field(event, "status")?)? != "observed"
                {
                    return Err("fault point was not independently observed".to_string());
                }
            }
            let observation_raw = fs::read(&receipt_path).map_err(|error| error.to_string())?;
            let reached: Result<Vec<String>, String> =
                array(field(observation, "observed_reached_points")?)?
                    .iter()
                    .map(|value| string(value).map(quote))
                    .collect();
            exact_inputs.push_str(&format!(
                ",\"fault_observation\":{},\"fault_schedule_identity\":{},\"harness_receipt_identity\":{},\"injection_points_reached\":[{}]",
                String::from_utf8(observation_raw.clone())
                    .map_err(|error| error.to_string())?
                    .trim_end(),
                quote(string(field(schedule_binding, "sha256")?)?),
                quote(&sha256(&observation_raw)),
                reached?.join(",")
            ));
        }
        if protocol == "semantic-receipt-v1" {
            let observation_value = load(&receipt_path)?;
            let observation = object(&observation_value)?;
            let closures = object(field(observation, "closures")?)?;
            let parity = object(field(observation, "promotion_parity")?)?;
            if string(field(observation, "schema_version")?)?
                != "maestro.external.vnext-final-semantic-artifact-readback.v1"
                || string(field(observation, "check_id")?)? != id
                || array(field(observation, "artifacts")?)?.is_empty()
                || array(field(observation, "canonical_reads")?)?.is_empty()
                || array(field(observation, "negative_routes")?)?.is_empty()
                || number(field(closures, "consumer_count")?)? != 0
                || number(field(closures, "reader_count")?)? != 0
                || number(field(closures, "hold_count")?)? != 0
                || number(field(parity, "source_file_count")?)? != 210
                || number(field(parity, "promoted_file_count")?)? != 210
                || number(field(parity, "mismatch_count")?)? != 0
            {
                return Err("promoted semantic receipt is absent or non-positive".to_string());
            }
            let observation_raw = fs::read(&receipt_path).map_err(|error| error.to_string())?;
            exact_inputs.push_str(&format!(
                ",\"harness_receipt_identity\":{},\"semantic_observation\":{}",
                quote(&sha256(&observation_raw)),
                String::from_utf8(observation_raw)
                    .map_err(|error| error.to_string())?
                    .trim_end()
            ));
        }
        if protocol == "cohort-observation-v1" {
            let cohort = object(field(harness, "cohort")?)?;
            let observation_value = load(&receipt_path)?;
            let observation = object(&observation_value)?;
            let executables = object(field(observation, "executables")?)?;
            let outcomes = object(field(observation, "outcomes")?)?;
            let expected_identity_keys: BTreeSet<String> = ["old_reader", "new_reader", "writer"]
                .into_iter()
                .map(str::to_string)
                .collect();
            let expected_outcome_keys: BTreeSet<String> =
                ["old_reader", "new_reader", "writer", "rollback"]
                    .into_iter()
                    .map(str::to_string)
                    .collect();
            if string(field(observation, "schema_version")?)?
                != "maestro.external.vnext-final-cohort-observation.v1"
                || string(field(observation, "proof_id")?)? != id
                || string(field(observation, "cohort_identity")?)?
                    != string(field(cohort, "sha256")?)?
                || executables.keys().cloned().collect::<BTreeSet<_>>() != expected_identity_keys
                || outcomes.keys().cloned().collect::<BTreeSet<_>>() != expected_outcome_keys
                || outcomes.values().any(|value| {
                    object(value).map_or(true, |row| {
                        field(row, "typed_result").is_err()
                            || field(row, "observation").and_then(object).is_err()
                    })
                })
            {
                return Err(
                    "migration harness did not emit typed cohort identities and outcomes"
                        .to_string(),
                );
            }
            let proof_output = Path::new(&args[7])
                .parent()
                .ok_or("engine output has no parent")?;
            for executable in executables.values() {
                let executable = object(executable)?;
                let root = match string(field(executable, "root")?)? {
                    "source" => source_path.to_path_buf(),
                    "target" => PathBuf::from(
                        env::var("CARGO_TARGET_DIR")
                            .map_err(|_| "CARGO_TARGET_DIR is absent".to_string())?,
                    ),
                    "output" => proof_output.to_path_buf(),
                    value => return Err(format!("unknown cohort executable root: {}", value)),
                };
                verify_binding(&root, executable)?;
            }
            for (route, outcome) in outcomes {
                let outcome = object(outcome)?;
                let observation_binding = object(field(outcome, "observation")?)?;
                verify_binding(proof_output, observation_binding)?;
                let route_path =
                    safe_relative(proof_output, string(field(observation_binding, "path")?)?)?;
                let route_value = load(&route_path)?;
                let route_observation = object(&route_value)?;
                if route_observation.len() != 5
                    || string(field(route_observation, "schema_version")?)?
                        != "maestro.external.vnext-final-cohort-route-observation.v1"
                    || string(field(route_observation, "proof_id")?)? != id
                    || string(field(route_observation, "route")?)? != route
                    || string(field(route_observation, "typed_result")?)?
                        != string(field(outcome, "typed_result")?)?
                    || string(field(route_observation, "status")?)? != "observed"
                {
                    return Err("cohort route was not independently observed".to_string());
                }
            }
            let observation_raw = fs::read(&receipt_path).map_err(|error| error.to_string())?;
            exact_inputs.push_str(&format!(
                ",\"cohort_identity\":{},\"cohort_observation\":{},\"harness_receipt_identity\":{}",
                quote(string(field(cohort, "sha256")?)?),
                String::from_utf8(observation_raw.clone())
                    .map_err(|error| error.to_string())?
                    .trim_end(),
                quote(&sha256(&observation_raw))
            ));
        }
        if kind == "ancestry" {
            let observation_value = load(&receipt_path)?;
            let observation = object(&observation_value)?;
            if string(field(observation, "schema_version")?)?
                != "maestro.external.vnext-final-fanout-edge-sweep.v1"
                || string(field(observation, "status")?)? != "pass"
                || array(field(observation, "edges")?)?.len() != 12
            {
                return Err("fanout edge sweep receipt differs".to_string());
            }
            let observation_raw = fs::read(&receipt_path).map_err(|error| error.to_string())?;
            exact_inputs.push_str(&format!(
                ",\"edge_sweep\":{},\"edge_sweep_identity\":{}",
                String::from_utf8(observation_raw.clone())
                    .map_err(|error| error.to_string())?
                    .trim_end(),
                quote(&sha256(&observation_raw))
            ));
        }
        proof_rows.push(format!(
            "{{\"actual_outcome\":{},\"command_identity\":{}{},\"expected_outcome\":{},\"exit_code\":{},\"kind\":{},\"produced_artifacts\":[],\"proof_id\":{},\"stage\":{},\"stderr\":{},\"stdout\":{}}}",
            quote(actual),
            quote(string(field(command, "identity")?)?),
            exact_inputs,
            quote(expected),
            exit,
            quote(string(field(proof, "kind")?)?),
            quote(id),
            number(field(proof, "stage")?)?,
            stream_json(&output.stderr),
            stream_json(&output.stdout)
        ));
    }
    let mut checks = Vec::new();
    let mut all_pass = true;
    let mut consumer_count = 0i64;
    let mut reader_count = 0i64;
    let mut hold_count = 0i64;
    for check in array(field(readback, "checks")?)? {
        let check = object(check)?;
        let check_command = {
            let mut command = BTreeMap::new();
            command.insert("argv".to_string(), field(check, "argv")?.clone());
            command.insert(
                "expected_exit_code".to_string(),
                field(check, "expected_exit_code")?.clone(),
            );
            command
        };
        if string(field(check, "command_identity")?)? != command_identity(&check_command)? {
            return Err("semantic readback command identity differs".to_string());
        }
        let receipt_path = Path::new(&args[7])
            .parent()
            .ok_or("engine output has no parent")?
            .join(format!("semantic-{}.v1.json", string(field(check, "id")?)?));
        let environment = BTreeMap::from([
            (
                "MAESTRO_SEMANTIC_READBACK_CHECK_ID".to_string(),
                string(field(check, "id")?)?.to_string(),
            ),
            (
                "MAESTRO_SEMANTIC_READBACK_RECEIPT".to_string(),
                receipt_path.to_string_lossy().to_string(),
            ),
        ]);
        let output = run(
            array(field(check, "argv")?)?,
            &args[6],
            &tools,
            &args[0],
            &args[5],
            &environment,
        )?;
        let exit = output.status.code().unwrap_or(-1) as i64;
        let artifact_value = load(&receipt_path)?;
        let artifact_receipt = object(&artifact_value)?;
        if string(field(artifact_receipt, "schema_version")?)?
            != "maestro.external.vnext-final-semantic-artifact-readback.v1"
            || string(field(artifact_receipt, "check_id")?)? != string(field(check, "id")?)?
        {
            return Err("semantic artifact receipt differs".to_string());
        }
        let mut artifact_kinds = BTreeSet::new();
        for artifact in array(field(artifact_receipt, "artifacts")?)? {
            let artifact = object(artifact)?;
            let root = match string(field(artifact, "root")?)? {
                "source" => source_path.to_path_buf(),
                "target" => PathBuf::from(
                    env::var("CARGO_TARGET_DIR")
                        .map_err(|_| "CARGO_TARGET_DIR is absent".to_string())?,
                ),
                "output" => Path::new(&args[7])
                    .parent()
                    .ok_or("engine output has no parent")?
                    .to_path_buf(),
                value => return Err(format!("unknown semantic artifact root: {}", value)),
            };
            verify_binding(&root, artifact)?;
            artifact_kinds.insert(string(field(artifact, "kind")?)?.to_string());
        }
        let required_kinds: Result<BTreeSet<String>, String> =
            array(field(check, "required_artifact_kinds")?)?
                .iter()
                .map(|value| string(value).map(str::to_string))
                .collect();
        if !required_kinds?.is_subset(&artifact_kinds) {
            return Err("semantic readback omitted required produced artifacts".to_string());
        }
        let reads = array(field(artifact_receipt, "canonical_reads")?)?;
        if reads.len() < number(field(check, "minimum_canonical_reads")?)? as usize
            || reads.iter().any(|value| {
                object(value)
                    .and_then(|row| {
                        if string(field(row, "status")?)? != "pass"
                            || !string(field(row, "command_identity")?)?.starts_with("sha256:")
                        {
                            return Err("canonical read differs".to_string());
                        }
                        Ok(())
                    })
                    .is_err()
            })
        {
            return Err("representative canonical reads are absent".to_string());
        }
        let proof_output = Path::new(&args[7])
            .parent()
            .ok_or("engine output has no parent")?;
        for read in reads {
            let read = object(read)?;
            let observation_binding = object(field(read, "observation")?)?;
            verify_binding(proof_output, observation_binding)?;
            let observation_path =
                safe_relative(proof_output, string(field(observation_binding, "path")?)?)?;
            let observation_value = load(&observation_path)?;
            let observation = object(&observation_value)?;
            if observation.len() != 5
                || string(field(observation, "schema_version")?)?
                    != "maestro.external.vnext-final-canonical-read-observation.v1"
                || string(field(observation, "check_id")?)? != string(field(check, "id")?)?
                || string(field(observation, "route")?)? != string(field(read, "route")?)?
                || string(field(observation, "command_identity")?)?
                    != string(field(read, "command_identity")?)?
                || string(field(observation, "status")?)? != "pass"
            {
                return Err("canonical read observation differs".to_string());
            }
        }
        let routes = array(field(artifact_receipt, "negative_routes")?)?;
        if routes.len() < number(field(check, "minimum_negative_routes")?)? as usize
            || routes.iter().any(|value| {
                object(value)
                    .and_then(|row| {
                        if !matches!(field(row, "injected")?, Value::Bool(true))
                            || string(field(row, "outcome")?)? != "refuse"
                            || !string(field(row, "receipt_identity")?)?.starts_with("sha256:")
                        {
                            return Err("negative route differs".to_string());
                        }
                        Ok(())
                    })
                    .is_err()
            })
        {
            return Err("negative route injections are absent".to_string());
        }
        for route in routes {
            let route = object(route)?;
            let observation_binding = object(field(route, "observation")?)?;
            verify_binding(proof_output, observation_binding)?;
            let observation_path =
                safe_relative(proof_output, string(field(observation_binding, "path")?)?)?;
            let observation_value = load(&observation_path)?;
            let observation = object(&observation_value)?;
            if observation.len() != 6
                || string(field(observation, "schema_version")?)?
                    != "maestro.external.vnext-final-negative-route-observation.v1"
                || string(field(observation, "check_id")?)? != string(field(check, "id")?)?
                || string(field(observation, "route")?)? != string(field(route, "route")?)?
                || !matches!(field(observation, "injected")?, Value::Bool(true))
                || string(field(observation, "outcome")?)? != "refuse"
                || string(field(observation, "receipt_identity")?)?
                    != string(field(route, "receipt_identity")?)?
            {
                return Err("negative route observation differs".to_string());
            }
        }
        let closures = object(field(artifact_receipt, "closures")?)?;
        let parity = object(field(artifact_receipt, "promotion_parity")?)?;
        if number(field(parity, "source_file_count")?)? != 210
            || number(field(parity, "promoted_file_count")?)? != 210
            || number(field(parity, "mismatch_count")?)? != 0
        {
            return Err("promotion parity differs".to_string());
        }
        let counts = (
            number(field(closures, "consumer_count")?)?,
            number(field(closures, "reader_count")?)?,
            number(field(closures, "hold_count")?)?,
        );
        if counts != (0, 0, 0) {
            return Err("semantic consumer, reader, or hold closure differs".to_string());
        }
        consumer_count = consumer_count.max(counts.0);
        reader_count = reader_count.max(counts.1);
        hold_count = hold_count.max(counts.2);
        let passed = exit == number(field(check, "expected_exit_code")?)?;
        all_pass &= passed;
        checks.push(format!(
            "{{\"artifact_receipt_identity\":{},\"command_identity\":{},\"consumer_count\":{},\"exit_code\":{},\"hold_count\":{},\"id\":{},\"kind\":{},\"reader_count\":{},\"status\":{}}}",
            quote(&sha256(
                &fs::read(&receipt_path).map_err(|error| error.to_string())?
            )),
            quote(string(field(check, "command_identity")?)?),
            counts.0,
            exit,
            counts.2,
            quote(string(field(check, "id")?)?),
            quote(string(field(check, "kind")?)?),
            counts.1,
            quote(if passed { "pass" } else { "fail" })
        ));
    }
    let snapshot_identity = sha256(&fs::read(snapshot_path).map_err(|error| error.to_string())?);
    let input_manifest_identity =
        sha256(&fs::read(manifest_path).map_err(|error| error.to_string())?);
    let ledger_identity = sha256(&fs::read(ledger_path).map_err(|error| error.to_string())?);
    let readback_identity = sha256(&fs::read(readback_path).map_err(|error| error.to_string())?);
    let toolchain_identity = sha256(&fs::read(toolchain_path).map_err(|error| error.to_string())?);
    let receipt = format!("{{\"engine\":\"rust\",\"input_manifest_identity\":{},\"ledger_identity\":{},\"proofs\":[{}],\"readback_plan_identity\":{},\"schema_version\":\"maestro.external.vnext-final-engine-ledger.v1\",\"semantic_readback\":{{\"checks\":[{}],\"consumer_count\":{},\"hold_count\":{},\"reader_count\":{},\"status\":{}}},\"snapshot_identity\":{},\"toolchain_identity\":{}}}\n", quote(&input_manifest_identity), quote(&ledger_identity), proof_rows.join(","), quote(&readback_identity), checks.join(","), consumer_count, hold_count, reader_count, quote(if all_pass {"pass"} else {"fail"}), quote(&snapshot_identity), quote(&toolchain_identity));
    fs::write(&args[7], receipt).map_err(|error| error.to_string())
}

fn main() {
    if let Err(error) = main_result() {
        eprintln!("{}", error);
        std::process::exit(1);
    }
}
