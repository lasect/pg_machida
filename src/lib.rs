use pgrx::prelude::*;

::pgrx::pg_module_magic!(name, version);
::pgrx::extension_sql_file!("../sql/pg_machida--0.1.0.sql", name = "schema", bootstrap);

pub mod types;
pub mod book;
pub mod engine;
pub mod matching;
pub mod state;
pub mod persistence;
pub mod background_worker;
pub mod notify;
pub mod error;

#[pg_extern]
fn hello_pg_machida() -> &'static str {
    "Hello, pg_machida"
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_hello_pg_machida() {
        assert_eq!("Hello, pg_machida", crate::hello_pg_machida());
    }
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
