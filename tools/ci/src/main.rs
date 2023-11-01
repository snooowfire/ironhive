use xshell::{cmd, Shell};

const CLIPPY_FLAGS: [&str; 6] = [
    "-Wclippy::doc_markdown",
    "-Wclippy::redundant_else",
    "-Wclippy::match_same_arms",
    "-Wclippy::semicolon_if_nothing_returned",
    "-Wclippy::map_flatten",
    "-Dwarnings",
];

fn main() {
    let flags = xflags::parse_or_exit! {
        /// Enables the execution of cargo test.
        ///
        /// Run tests (except doc tests and without building examples)
        optional --test

        /// Enables the execution of cargo check.
        optional --check

        /// Enables the execution of cargo clippy.
        ///
        /// See if clippy has any complaints.
        /// - Type complexity must be ignored because we use huge templates for queries
        optional --clippy

        /// Enables the execution of cargo fmt.
        ///
        /// See if any code needs to be formatted
        optional --format
    };

    let sh = Shell::new().unwrap();

    if flags.format {
        cmd!(sh, "cargo fmt --all -- --check")
            .run()
            .expect("Please run 'cargo fmt --all' to format your code.");
    }

    if flags.clippy {
        cmd!(
            sh,
            "cargo clippy --workspace --all-targets --all-features -- {CLIPPY_FLAGS...}"
        )
        .run()
        .expect("Please fix clippy errors in output above.");
    }

    if flags.test {
        macro_rules! test_windows {
            ($($test: path),*) => {
            #[cfg(windows)]
            {
                $(
                    let test = stringify!($test);

                    cmd!(sh,"cargo test --package ironhive-core --lib -- {test} --exact --nocapture")
                        .run()
                        .expect("Please fix failing tests in output above.");
                )*
            }
            };
        }

        test_windows!(
            windows::wua::test_updates,
            windows::wmi::test_wmi,
            windows::svc::test_description,
            windows::svc::test_get_config,
            windows::svc::test_get_service,
            windows::svc::test_mgr,
            utils::test_public_ip,
            windows::svc::test_installed_software_list,
            agent::tests::test_agent
        );
        #[cfg(windows)]
        cmd!(
            sh,
            "cargo test --workspace --lib --bins --tests --benches --exclude ironhive-core"
        )
        .run()
        .expect("Please fix failing tests in output above.");

        #[cfg(not(windows))]
        cmd!(sh, "cargo test --workspace --lib --bins --tests --benches")
            .run()
            .expect("Please fix failing tests in output above.");
    }

    if flags.check {
        cmd!(sh, "cargo check --workspace")
            .run()
            .expect("Please fix compiler errors in output above.");
    }
}
