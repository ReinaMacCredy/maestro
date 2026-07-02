use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub fn maestro(cwd: &Path) -> MaestroCmd {
    MaestroCmd {
        cwd: cwd.to_path_buf(),
        args: Vec::new(),
        envs: Vec::new(),
        stdin: None,
    }
}

pub struct MaestroCmd {
    cwd: PathBuf,
    args: Vec<OsString>,
    envs: Vec<(OsString, OsString)>,
    stdin: Option<Vec<u8>>,
}

impl MaestroCmd {
    #[allow(dead_code, reason = "integration crates use different harness slices")]
    pub fn args(mut self, args: &[&str]) -> Self {
        self.args
            .extend(args.iter().map(|arg| OsString::from(*arg)));
        self
    }

    #[allow(dead_code, reason = "integration crates use different harness slices")]
    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    #[allow(dead_code, reason = "integration crates use different harness slices")]
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.envs
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    #[allow(dead_code, reason = "integration crates use different harness slices")]
    pub fn stdin(mut self, text: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(text.into());
        self
    }

    #[allow(dead_code, reason = "integration crates use different harness slices")]
    pub fn output(self) -> MaestroOutput {
        let mut command = self.command();
        let stdin = self.stdin;
        if stdin.is_none() {
            return MaestroOutput::new(command.output().expect(
                "invariant: compiled maestro binary should be runnable in integration tests",
            ));
        }

        if stdin.is_some() {
            command.stdin(Stdio::piped());
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .expect("invariant: compiled maestro binary should spawn in integration tests");
        if let Some(stdin) = stdin {
            child
                .stdin
                .take()
                .expect("invariant: stdin should be piped")
                .write_all(&stdin)
                .expect("invariant: stdin should be writable");
        }

        let output = child
            .wait_with_output()
            .expect("invariant: maestro process should exit");
        MaestroOutput::new(output)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
        command.args(&self.args).current_dir(&self.cwd);
        for (key, value) in &self.envs {
            command.env(key, value);
        }
        command
    }
}

pub struct MaestroOutput {
    output: Output,
}

impl MaestroOutput {
    fn new(output: Output) -> Self {
        Self { output }
    }

    #[allow(dead_code, reason = "integration crates use different harness slices")]
    pub fn stdout(&self) -> String {
        String::from_utf8(self.output.stdout.clone()).expect("invariant: stdout should be UTF-8")
    }

    #[allow(dead_code, reason = "integration crates use different harness slices")]
    pub fn stderr(&self) -> String {
        String::from_utf8(self.output.stderr.clone()).expect("invariant: stderr should be UTF-8")
    }

    #[allow(dead_code, reason = "integration crates use different harness slices")]
    pub fn assert_success(&self, context: impl Display) {
        assert!(
            self.output.status.success(),
            "{context} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
    }

    #[allow(dead_code, reason = "integration crates use different harness slices")]
    pub fn into_raw(self) -> Output {
        self.output
    }
}
