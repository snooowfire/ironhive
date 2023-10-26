use std::path::PathBuf;
use std::process::Command;
use xshell::{cmd, Shell};

fn main() -> anyhow::Result<()> {
    let flags = xflags::parse_or_exit! {
        /// File or directory to remove
        required python3: PathBuf
    };

    let sh = Shell::new()?;

    // run server
    let server = nats_server::run_basic_server();

    if !sh.path_exists("./bin") {
        sh.create_dir("./bin")?;
    }

    let cli = {
        #[cfg(windows)]
        {
            "./bin/ironhive.exe"
        }
        #[cfg(not(windows))]
        {
            "./bin/ironhive"
        }
    };

    // build cli
    if !sh.path_exists(cli) {
        cmd!(sh, "cargo build -r").run()?;
        sh.copy_file(
            {
                #[cfg(windows)]
                {
                    "./target/release/ironhive.exe"
                }
                #[cfg(not(windows))]
                {
                    "./target/release/ironhive"
                }
            },
            cli,
        )?;
    }

    let python = {
        #[cfg(windows)]
        {
            "./bin/run_python.exe"
        }
        #[cfg(not(windows))]
        {
            "./bin/run_python"
        }
    };

    // build run python
    if !sh.path_exists(python) {
        cmd!(sh, "cargo build -r --example run_python").run()?;
        sh.copy_file(
            {
                #[cfg(windows)]
                {
                    "./target/release/examples/run_python.exe"
                }
                #[cfg(not(windows))]
                {
                    "./target/release/examples/run_python"
                }
            },
            python,
        )?;
    }

    let checker = {
        #[cfg(windows)]
        {
            "./bin/run_python_checker.exe"
        }
        #[cfg(not(windows))]
        {
            "./bin/run_python_checker"
        }
    };

    // build run python checker
    if !sh.path_exists(checker) {
        cmd!(sh, "cargo build -r --example run_python_checker --features deserialize").run()?;

        sh.copy_file(
            {
                #[cfg(windows)]
                {
                    "./target/release/examples/run_python_checker.exe"
                }
                #[cfg(not(windows))]
                {
                    "./target/release/examples/run_python_checker"
                }
            },
            checker,
        )?;
    }

    let server_url = server.client_url();

    cmd!(sh, "{cli} install -n {server_url} --overwrite-config").run()?;

    std::thread::sleep(std::time::Duration::from_secs(2));

    let mut ironhive = Command::new(cli).args(["rpc"]).spawn()?;

    std::thread::sleep(std::time::Duration::from_secs(3));

    let reply = "ironhive_run_python";

    let id = 1.to_string();
    let timeout = 10.to_string();

    let python3 = flags.python3;
    let expect = "The value of the 10th term in the Fibonacci sequence is: 34";

    let mut checker = Command::new(checker)
        .args(["--reply", reply, "--id", id.as_str(), "--expect", expect])
        .spawn()?;

    std::thread::sleep(std::time::Duration::from_secs(3));

    cmd!(sh, "{python} --reply {reply} --python {python3} --timeout {timeout} --id {id}").run()?;

    assert!(checker.wait()?.success());

    ironhive.kill()?;

    Ok(())
}
