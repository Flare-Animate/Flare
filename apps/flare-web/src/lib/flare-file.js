// Reading and writing a .flare file in the page.
//
// A .flare file is a SQLite database whose records are zstd-compressed CBOR (docs/ARCHITECTURE.md,
// "Storage"). A page has neither SQLite nor zstd, so it brings them as WebAssembly
// (@sqlite.org/sqlite-wasm and @bokuweb/zstd-wasm) and this module is all it needs of them: it opens
// and builds the database and compresses, and hands the records to Rust as plain rows. Everything
// that decides whether the rows are good (digests, sizes, frame headers, the document's own
// validation) is Rust's, the same code the desktop app runs.
//
// The two libraries are parameters, so the browser and a test under Node build it the same way:
//   const file = createFlareFile({ sqlite3, zstd });

const APPLICATION_ID = 0x464c4152; // "FLAR"
const PAGE_SIZE = 1024;
const ZSTD_LEVEL = 3;

export function createFlareFile({ sqlite3, zstd }) {
  const { capi, wasm, oo1 } = sqlite3;

  // Opens `bytes` (a whole database file) as an in-memory database.
  function open(bytes) {
    const db = new oo1.DB(":memory:", "c");
    try {
      const memory = wasm.allocFromTypedArray(bytes);
      const rc = capi.sqlite3_deserialize(
        db.pointer,
        "main",
        memory,
        bytes.length,
        bytes.length,
        capi.SQLITE_DESERIALIZE_FREEONCLOSE,
      );
      if (rc !== 0) {
        throw new Error(`SQLite would not open the file (code ${rc})`);
      }
      // A file from outside is untrusted: no schema tricks, no writes to the structure.
      capi.sqlite3_db_config(db.pointer, capi.SQLITE_DBCONFIG_DEFENSIVE, 1, 0);
      capi.sqlite3_db_config(db.pointer, capi.SQLITE_DBCONFIG_TRUSTED_SCHEMA, 0, 0);
      return db;
    } catch (error) {
      db.close();
      throw error;
    }
  }

  const one = (db, sql) => db.selectValue(sql);

  return {
    // The records of a .flare file as plain rows, or an Error saying it is not one.
    read(bytes) {
      if (!(bytes instanceof Uint8Array)) {
        throw new Error("a file must be bytes");
      }
      const db = open(bytes);
      try {
        let applicationId;
        try {
          applicationId = one(db, "PRAGMA application_id");
        } catch {
          throw new Error("not a Flare project file");
        }
        if (applicationId !== APPLICATION_ID) {
          throw new Error("not a Flare project file");
        }
        // Which versions this build reads is Rust's to say (flare-storage's check_schema_version).
        const schemaVersion = one(db, "PRAGMA user_version");
        const project = db.selectObject(
          "SELECT project_id, settings_json FROM project WHERE singleton = 1",
        );
        if (!project) {
          throw new Error("the file has no project row");
        }
        const records = db.selectObjects(
          "SELECT id, kind, ordinal, codec, size, digest, payload FROM entity ORDER BY kind, ordinal",
        );
        return {
          schemaVersion,
          projectId: project.project_id,
          settings: project.settings_json,
          records: records.map((r) => ({
            id: r.id,
            kind: r.kind,
            ordinal: Number(r.ordinal),
            codec: r.codec,
            size: Number(r.size),
            digest: r.digest,
            payload: r.payload,
          })),
        };
      } finally {
        db.close();
      }
    },

    // The bytes of a .flare file holding these rows ({ schemaVersion, tables, projectId, settings,
    // records }). The version and the tables come from Rust (flare-storage's schema_version and
    // TABLES), so the page writes exactly what the desktop app does and cannot fall behind it.
    write(parts) {
      if (!Number.isInteger(parts.schemaVersion) || typeof parts.tables !== "string") {
        throw new Error("the rows to write need their schema version and tables");
      }
      const db = new oo1.DB(":memory:", "c");
      const version = parts.schemaVersion;
      try {
        // The page size can only be chosen before the first table exists.
        db.exec(`PRAGMA page_size = ${PAGE_SIZE}`);
        db.exec(`PRAGMA application_id = ${APPLICATION_ID}`);
        db.exec(`PRAGMA user_version = ${version}`);
        db.transaction(() => {
          db.exec(parts.tables);
          db.exec({
            sql: "INSERT INTO project VALUES (1, ?, ?, ?)",
            bind: [parts.projectId, version, parts.settings],
          });
          const insert = db.prepare("INSERT INTO entity VALUES (?, ?, ?, 1, ?, ?, ?, ?)");
          try {
            for (const r of parts.records) {
              insert.bind([r.id, r.kind, r.ordinal, r.codec, r.size, r.digest, r.payload]).step();
              insert.reset(true);
            }
          } finally {
            insert.finalize();
          }
        });
        return capi.sqlite3_js_db_export(db.pointer);
      } finally {
        db.close();
      }
    },

    zstdCompress: (bytes) => zstd.compress(bytes, ZSTD_LEVEL),
    zstdDecompress: (bytes) => zstd.decompress(bytes),
  };
}
