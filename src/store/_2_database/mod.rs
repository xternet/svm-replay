use super::*;

pub struct Store {
    pub(super) root: PathBuf,
    pub(super) db: Connection,
}
impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root.join("blobs"))?;
        let db = Connection::open(root.join("index.sqlite"))?;
        db.busy_timeout(Duration::from_secs(10))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS meta (id INTEGER PRIMARY KEY CHECK(id=1), schema_version INTEGER NOT NULL, seq INTEGER NOT NULL);
            INSERT OR IGNORE INTO meta VALUES(1,1,0);
            CREATE TABLE IF NOT EXISTS entries(namespace TEXT NOT NULL, key TEXT NOT NULL, hash TEXT NOT NULL, size INTEGER NOT NULL, accessed INTEGER NOT NULL, PRIMARY KEY(namespace,key));
            CREATE TABLE IF NOT EXISTS pins(namespace TEXT NOT NULL,key TEXT NOT NULL,owner TEXT NOT NULL,PRIMARY KEY(namespace,key,owner),FOREIGN KEY(namespace,key) REFERENCES entries(namespace,key) ON DELETE CASCADE);
            CREATE TABLE IF NOT EXISTS leases(namespace TEXT NOT NULL,key TEXT NOT NULL,owner TEXT NOT NULL,expires INTEGER NOT NULL,released INTEGER NOT NULL CHECK(released IN(0,1)),PRIMARY KEY(namespace,key,owner));
            CREATE TABLE IF NOT EXISTS budgets(id TEXT PRIMARY KEY,limit_reads INTEGER NOT NULL,limit_requests INTEGER NOT NULL,limit_bytes INTEGER NOT NULL,used_reads INTEGER NOT NULL,used_requests INTEGER NOT NULL,used_bytes INTEGER NOT NULL);")?;
        let schema: i64 =
            db.query_row("SELECT schema_version FROM meta WHERE id=1", [], |row| {
                row.get(0)
            })?;
        if schema != 1 {
            return Err(StoreError::new(
                "SCHEMA_UNSUPPORTED",
                format!("store schema {schema} is not supported"),
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            db,
        })
    }
}
