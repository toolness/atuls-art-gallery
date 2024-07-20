pub use index_file::index_wikidata_dump;
pub use query::{execute_wikidata_query, prepare_wikidata_query};

mod index_file;
mod query;
mod sledcache;
mod sparql_csv_export;

const BUFREADER_CAPACITY: usize = 1024 * 1024 * 8;
