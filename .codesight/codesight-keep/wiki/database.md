# Database

> **Navigation aid.** Schema shapes and field types extracted via AST. Read the actual schema source files before writing migrations or query logic.

**unknown** — 5 models

### project

pk: `singleton` (integer) · fk: project_id

- `singleton`: integer _(pk)_
- `project_id`: text _(required, fk)_
- `schema_version`: integer _(required)_
- `saved_revision`: integer _(required)_
- `title`: text _(required)_
- `settings_json`: text _(required)_
- `created_utc`: text _(required)_
- `saved_utc`: text _(required)_
- `kind`: text _(required)_
- `payload_version`: integer _(required)_
- `payload_json`: text _(required)_

### entity_reference

fk: owner_id, target_id

- `owner_id`: text _(required, fk)_
- `field_path`: text _(required)_
- `target_id`: text _(required, fk)_

### blob

pk: `digest` (bytes)

- `digest`: bytes _(pk)_
- `codec`: text _(required)_
- `logical_bytes`: integer _(required)_
- `stored_bytes`: integer _(required)_
- `payload`: bytes _(required)_

### asset

pk: `id` (text) · fk: embedded_digest

- `id`: text _(pk)_
- `media_type`: text _(required)_
- `embedded_digest`: bytes _(fk)_
- `linked_uri`: text
- `metadata_json`: text _(required)_

### source_attachment

pk: `id` (text) · fk: anchor_entity_id

- `id`: text _(pk)_
- `source_format`: text _(required)_
- `source_profile`: text _(required)_
- `anchor_entity_id`: text _(fk)_
- `source_path`: text _(required)_
- `content_digest`: bytes _(required)_
- `preservation_mode`: text _(required)_
- `validity`: text _(required)_
- `dependency_manifest_json`: text _(required)_

## Schema Source Files

Search for ORM schema declarations:
- Drizzle: `pgTable` / `mysqlTable` / `sqliteTable`
- Prisma: `prisma/schema.prisma`
- TypeORM: `@Entity()` decorator
- SQLAlchemy: class inheriting `Base`

---
_Back to [overview.md](./overview.md)_