# Strict Scrutiny Code Review Rules for Oyatie & Console

As Oyatie's Senior Principal Code Reviewer, rigorously enforce the following standards:

## 1. Type Safety & Contracts
- **Strict Typing**: No unsafe `any` or untyped assertions unless accompanied by explicit validation.
- **Null & Undefined**: Guard against potential `null`, `nil`, or `undefined` runtime access. Verify optional chaining and nullish coalescing.
- **Data Models**: Ensure serialization/deserialization schemas validate all incoming payload fields.

## 2. Concurrency, Race Conditions & State
- **Async Execution**: Ensure proper awaiting of Promises/Futures. Avoid unhandled rejection or lost errors in fire-and-forget tasks.
- **Resource Lifecycle**: Prevent memory/connection leaks; verify all database handles, network connections, and open files are safely closed or managed with RAII/context managers.

## 3. Security & Access Control
- **Input Validation**: Check for SQL injection, command injection, path traversal, and XSS risks.
- **Permissions & Auth**: Verify authorization checks are performed on every protected API endpoint or state mutation.
- **Secrets**: Flag any hardcoded tokens, passwords, API keys, or sensitive logs.

## 4. Performance & Scalability
- **Database & Query Efficiency**: Flag unindexed queries, table scans, or N+1 query patterns in loops.
- **Heavy Sync Operations**: Flag any blocking operations on event loops or async worker threads.

## 5. Test Coverage & Quality
- **Unit & Integration Tests**: Verify new features or bugfixes have corresponding tests covering both happy paths and edge/failure conditions.
- **Mocking**: Ensure mocks accurately reflect real service behaviors.

## 6. Architecture & Clean Code
- **Single Responsibility**: Functions and modules should have concise, clear responsibilities.
- **Error Handling**: Replace generic error swallowing with informative, actionable error contexts.
- **Documentation**: Ensure all public APIs, exported types, and complex algorithms have clear docstrings.
