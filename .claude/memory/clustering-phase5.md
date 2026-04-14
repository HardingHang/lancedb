---
name: LanceDB Clustering Phase 5
description: Phase 5 complete - Node.js bindings for clustering with 5 integration tests passing
type: project
originSessionId: f2184b1a-66e3-4a0d-9a56-42f2025f4801
---
Node.js/TypeScript bindings for the multi-dimensional clustering feature are complete.

**Why:** Phase 5 was the next step after completing Python bindings (Phase 4) to make clustering accessible to Node.js users.

**What was done:**
- Exposed `clusterBy` parameter on `Connection.createTable` / `createEmptyTable`
- Exposed `Table.clusterConfig()` returning `{ keys: string[], algorithm: string, algorithmParams?: unknown } | undefined`
- Exposed `Table.cluster(options?: { targetRowsPerFragment?: number })` returning clustering stats
- Added napi-rs bindings in `nodejs/src/table.rs` and `nodejs/src/connection.rs`
- Added TypeScript layer methods in `nodejs/lancedb/table.ts`, `nodejs/lancedb/connection.ts`, and `nodejs/lancedb/index.ts`
- Added 5 integration tests in `nodejs/__test__/table.test.ts`
- Auto-generated `native.d.ts` updated via `npm run build`

**Test results:**
- Node.js `test_table.py`: 217/217 passed (including 5 new cluster tests)
- Node.js `connection.test.ts`: 15/15 passed
- `npm run lint` and `npm run lint-fix` passed

**Critical files changed:**
- `nodejs/src/table.rs`
- `nodejs/src/connection.rs`
- `nodejs/lancedb/table.ts`
- `nodejs/lancedb/connection.ts`
- `nodejs/lancedb/index.ts`
- `nodejs/__test__/table.test.ts`
- `nodejs/lancedb/native.d.ts` (auto-generated)
