---
name: TDD Core Principles
description: Test-Driven Development iron law and red-green-refactor cycle rules from obra/superpowers
type: feedback
originSessionId: e8b21aee-a8f9-4a27-9661-d8015fabaf6c
---
**Iron Law:** "NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST" — write code before test? "Delete it. Start over."

**Red-Green-Refactor Cycle:**
- **RED:** Write one minimal failing test with clear name
- **Verify RED:** "MANDATORY. Never skip" — confirm test fails correctly
- **GREEN:** "Minimal code" to pass — no over-engineering
- **Verify GREEN:** Confirm pass, no regressions, pristine output
- **REFACTOR:** Clean up while staying green

**Critical Rules:**
- "Violating the letter of the rules is violating the spirit"
- "If you didn't watch the test fail, you don't know if it tests the right thing"
- "Delete means delete" — no keeping as "reference"
- "Thinking 'skip TDD just this once'? Stop. That's rationalization"

**Red Flags (Stop & Restart):**
- Code before test
- Test passes immediately
- "I'll test after"
- "Keep as reference"
- "Already manually tested"
- "Deleting X hours is wasteful"

**Verification Checklist:**
- Every function tested
- Watched each fail
- Minimal passing code
- All green
- Pristine output
- Real code not mocks
- Edge cases covered

**Final Rule:** "Production code → test exists and failed first. Otherwise → not TDD."

**Why:** Following TDD prevents testing anti-patterns and ensures test coverage from the start.
**How to apply:** Before writing any production code, write a failing test first, watch it fail, then implement minimal code to pass.