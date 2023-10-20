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
            "./bin/cli.exe"
        }
        #[cfg(not(windows))]
        {
            "./bin/cli"
        }
    };

    // build cli
    if !sh.path_exists(cli) {
        cmd!(sh, "cargo build -r --example cli").run()?;
        sh.copy_file(
            {
                #[cfg(windows)]
                {
                    "./target/release/examples/cli.exe"
                }
                #[cfg(not(windows))]
                {
                    "./target/release/examples/cli"
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
        cmd!(
            sh,
            "cargo build -r --example run_python_checker --features deserialize"
        )
        .run()?;

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
    let agent_id = uuid::Uuid::new_v4().to_string();

    let mut ironhive = Command::new(cli)
        .args([
            "--agent-id",
            agent_id.as_str(),
            "--server",
            server_url.as_str(),
        ])
        .spawn()?;

    std::thread::sleep(std::time::Duration::from_secs(2));

    let reply = "ironhive_run_python";

    let id = 1.to_string();
    let timeout = 10.to_string();

    let code = r#"
def fibonacci(n):
    if n <= 0:
        return "Please enter a positive integer."
    elif n == 1:
        return 0
    elif n == 2:
        return 1
    else:
        a, b = 0, 1
        for _ in range(3, n+1):
            a, b = b, a + b
        return b

n = 10
result = fibonacci(n)
print(f"The value of the {n}th term in the Fibonacci sequence is: {result}")
                    "#;

    let python3 = flags.python3;

    let expect = "The value of the 10th term in the Fibonacci sequence is: 34";
    let mut checker = Command::new(checker)
        .args([
            "--server",
            server_url.as_str(),
            "--reply",
            reply,
            "--id",
            id.as_str(),
            "--expect",
            expect,
        ])
        .spawn()?;

    cmd!(sh,"{python} --agent-id {agent_id} --server {server_url} --reply {reply} --python {python3} --timeout {timeout} --id {id} --code {code}").run()?;

    assert!(checker.wait()?.success());

    ironhive.kill()?;

    Ok(())
}
