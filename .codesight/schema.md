# Schema

### project
- singleton: integer (pk)
- project_id: text (required, fk)
- schema_version: integer (required)
- saved_revision: integer (required)
- title: text (required)
- settings_json: text (required)
- created_utc: text (required)
- saved_utc: text (required)
- kind: text (required)
- payload_version: integer (required)
- payload_json: text (required)

### entity_reference
- owner_id: text (required, fk)
- field_path: text (required)
- target_id: text (required, fk)

### blob
- digest: bytes (pk)
- codec: text (required)
- logical_bytes: integer (required)
- stored_bytes: integer (required)
- payload: bytes (required)

### asset
- id: text (pk)
- media_type: text (required)
- embedded_digest: bytes (fk)
- linked_uri: text
- metadata_json: text (required)

### source_attachment
- id: text (pk)
- source_format: text (required)
- source_profile: text (required)
- anchor_entity_id: text (fk)
- source_path: text (required)
- content_digest: bytes (required)
- preservation_mode: text (required)
- validity: text (required)
- dependency_manifest_json: text (required)
